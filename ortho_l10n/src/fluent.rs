//! Fluent bundle construction and the localizer builder.
//!
//! Bundle construction, builder configuration, and the crate-internal access
//! needed to apply the isolation override live here. Identifier normalization
//! lives in [`crate::fluent_identifiers`] and is re-exported below so callers
//! keep the established `crate::fluent` paths.

use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
use std::fmt;
use std::sync::Arc;
use unic_langid::LanguageIdentifier;

use super::{
    FluentBundleSource, FluentLocalizer, FluentLocalizerBuilder, FluentLocalizerError,
    FormattingIssueReporter,
};

/// Keeps each Fluent bundle paired with the locale and source needed for
/// localized lookup and diagnostics.
pub(super) struct BundleWithLocale {
    /// Locale used to construct the bundle and reported with formatting issues.
    pub(super) locale: LanguageIdentifier,
    /// Shared parsed resources, retained behind `Arc` for `Fluent`'s concurrent bundle API.
    pub(super) bundle: FluentBundle<Arc<FluentResource>>,
    /// Origin tag used to distinguish consumer catalogues from embedded defaults.
    pub(super) kind: FluentBundleSource,
}

impl BundleWithLocale {
    /// Applies `Fluent`'s bidirectional isolation setting to the wrapped bundle.
    ///
    /// Access stays inside the crate so consumers configure behaviour through
    /// [`FluentLocalizerBuilder::with_use_isolating`] rather than by reaching
    /// into the localizer's bundles.
    pub(super) fn set_use_isolating(&mut self, value: bool) {
        self.bundle.set_use_isolating(value);
    }
}

impl fmt::Debug for BundleWithLocale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BundleWithLocale")
            .field("locale", &self.locale)
            .field("bundle", &"<fluent bundle>")
            .field("kind", &self.kind)
            .finish()
    }
}

/// Embedded English catalogue used for any requested locale whose language is English.
const EN_US_CATALOGUE: &str = include_str!("../locales/en-US/messages.ftl");
/// Embedded Japanese catalogue used for any requested locale whose language is Japanese.
const JA_CATALOGUE: &str = include_str!("../locales/ja/messages.ftl");

/// Resource slice for the embedded English catalogue, kept static for bundle construction.
static EN_US_RESOURCES: [&str; 1] = [EN_US_CATALOGUE];
/// Resource slice for the embedded Japanese catalogue, kept static for bundle construction.
static JA_RESOURCES: [&str; 1] = [JA_CATALOGUE];

/// Selects an embedded catalogue by language subtag; unsupported languages have no default.
pub(super) fn default_resources(locale: &LanguageIdentifier) -> Option<&'static [&'static str]> {
    match locale.language.as_str() {
        "en" => Some(&EN_US_RESOURCES),
        "ja" => Some(&JA_RESOURCES),
        _ => None,
    }
}

/// Parses, normalizes, and registers one resource set while preserving its origin for errors.
///
/// Parsing or duplicate-ID registration failures are returned with the requested locale
/// and catalogue source so callers can distinguish consumer failures from crate defaults.
pub(super) fn bundle_from_resources(
    locale: &LanguageIdentifier,
    resources: &[&'static str],
    catalogue: FluentBundleSource,
) -> Result<BundleWithLocale, FluentLocalizerError> {
    let mut bundle = FluentBundle::new_concurrent(vec![locale.clone()]);
    for resource in resources {
        let parsed = Arc::new(
            FluentResource::try_new(normalize_resource_ids(resource)).map_err(
                |(_resource, errors)| FluentLocalizerError::Parser {
                    locale: locale.clone(),
                    catalogue,
                    errors,
                },
            )?,
        );

        bundle
            .add_resource(parsed)
            .map_err(|errors| FluentLocalizerError::Registration {
                locale: locale.clone(),
                catalogue,
                errors,
            })?;
    }

    Ok(BundleWithLocale {
        locale: locale.clone(),
        bundle,
        kind: catalogue,
    })
}

pub(super) use crate::fluent_identifiers::{normalize_identifier, normalize_resource_ids};

impl FluentLocalizerBuilder {
    /// Creates a builder for the requested locale.
    #[must_use]
    pub fn new(locale: LanguageIdentifier) -> Self {
        Self {
            locale,
            consumer_resources: Vec::new(),
            consumer_bundle: None,
            report_issue: super::default_reporter(),
            use_defaults: true,
            use_isolating: None,
        }
    }

    /// Sets whether `Fluent` isolates interpolated values with Unicode control
    /// characters.
    ///
    /// `Fluent` wraps every interpolated value in U+2068 FIRST STRONG ISOLATE
    /// and U+2069 POP DIRECTIONAL ISOLATE so bidirectional algorithms keep
    /// right-to-left substitutions in the surrounding left-to-right text. Those
    /// characters are invisible in many terminals but are ordinary characters
    /// to `grep`, snapshot tests, and log pipelines.
    ///
    /// Leaving this unset preserves each bundle's own setting: resource-built
    /// bundles keep `Fluent`'s `true` default, and a supplied prebuilt bundle
    /// keeps whatever it chose. Passing `false` removes the marks from both the
    /// consumer catalogue and the embedded defaults, including the built-in
    /// Clap error copy. Passing `true` restores them on a bundle that was built
    /// without them, such as a supplied
    /// [`FluentBundle`](fluent_bundle::FluentBundle) whose own
    /// `set_use_isolating(false)` call the localizer would otherwise honour.
    ///
    /// The value is applied once, when [`Self::try_build`] constructs the
    /// bundles, so earlier and later calls to this setter have the same effect
    /// as the last call. Supplying a prebuilt consumer bundle still replaces
    /// any consumer resource strings; this setting then applies to that bundle.
    ///
    /// This is not general bidirectional sanitization. It controls one `Fluent`
    /// formatting option and never rewrites catalogue text.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_l10n::{FluentLocalizer, FluentValue, LocalizationArgs, Localizer, langid};
    ///
    /// let localizer = FluentLocalizer::builder(langid!("en-US"))
    ///     .with_use_isolating(false)
    ///     .with_consumer_resources(["welcome = Hello, { $name }!"])
    ///     .try_build()
    ///     .expect("valid catalogue");
    ///
    /// let mut args = LocalizationArgs::new();
    /// args.insert("name", FluentValue::from("Ada"));
    ///
    /// assert_eq!(
    ///     localizer.lookup("welcome", Some(&args)).as_deref(),
    ///     Some("Hello, Ada!"),
    /// );
    /// ```
    #[must_use]
    pub const fn with_use_isolating(mut self, value: bool) -> Self {
        self.use_isolating = Some(value);
        self
    }

    /// Adds consumer-provided Fluent resources to layer over the defaults.
    #[must_use]
    pub fn with_consumer_resources(
        mut self,
        resources: impl IntoIterator<Item = &'static str>,
    ) -> Self {
        self.consumer_resources.extend(resources);
        self
    }

    /// Supplies a pre-built consumer bundle, bypassing resource parsing.
    #[must_use]
    pub fn with_consumer_bundle(mut self, bundle: FluentBundle<Arc<FluentResource>>) -> Self {
        self.consumer_bundle = Some(BundleWithLocale {
            locale: self.locale.clone(),
            bundle,
            kind: FluentBundleSource::Consumer,
        });
        self
    }

    /// Disables loading embedded defaults, enabling consumer-only catalogues.
    #[must_use]
    pub const fn disable_defaults(mut self) -> Self {
        self.use_defaults = false;
        self
    }

    /// Installs a hook to report formatting issues surfaced by Fluent.
    #[must_use]
    pub fn with_error_reporter(mut self, reporter: FormattingIssueReporter) -> Self {
        self.report_issue = reporter;
        self
    }

    /// Builds the [`FluentLocalizer`], validating both default and consumer bundles.
    ///
    /// # Errors
    ///
    /// Returns [`FluentLocalizerError`] if any catalogue fails to parse or
    /// registers conflicting identifiers.
    pub fn try_build(self) -> Result<FluentLocalizer, FluentLocalizerError> {
        let mut defaults = self.build_defaults_bundle()?;
        let mut consumer = Self::build_consumer_bundle(
            &self.locale,
            self.consumer_bundle,
            &self.consumer_resources,
        )?;

        // Apply the isolation override once, here, so fallback lookups and the
        // built-in Clap error copy cannot disagree with the consumer catalogue
        // and the builder's setter order stays irrelevant.
        if let Some(value) = self.use_isolating {
            for bundle in [defaults.as_mut(), consumer.as_mut()].into_iter().flatten() {
                bundle.set_use_isolating(value);
            }
        }

        Ok(FluentLocalizer {
            consumer,
            defaults,
            report_issue: self.report_issue,
        })
    }

    /// Build the built-in defaults bundle when defaults are enabled.
    fn build_defaults_bundle(&self) -> Result<Option<BundleWithLocale>, FluentLocalizerError> {
        if !self.use_defaults {
            return Ok(None);
        }
        let resources = default_resources(&self.locale).ok_or_else(|| {
            FluentLocalizerError::UnsupportedLocale {
                locale: self.locale.clone(),
            }
        })?;
        Ok(Some(bundle_from_resources(
            &self.locale,
            resources,
            FluentBundleSource::Default,
        )?))
    }

    /// Build the consumer bundle from an explicit bundle or gathered resources.
    fn build_consumer_bundle(
        locale: &LanguageIdentifier,
        explicit: Option<BundleWithLocale>,
        resources: &[&'static str],
    ) -> Result<Option<BundleWithLocale>, FluentLocalizerError> {
        if let Some(bundle) = explicit {
            return Ok(Some(bundle));
        }
        if resources.is_empty() {
            return Ok(None);
        }
        Ok(Some(bundle_from_resources(
            locale,
            resources,
            FluentBundleSource::Consumer,
        )?))
    }
}

impl fmt::Debug for FluentLocalizerBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FluentLocalizerBuilder")
            .field("locale", &self.locale)
            .field("consumer_resources_len", &self.consumer_resources.len())
            .field(
                "consumer_bundle",
                &self.consumer_bundle.as_ref().map(|bundle| &bundle.locale),
            )
            .field("use_defaults", &self.use_defaults)
            .field("use_isolating", &self.use_isolating)
            .field("report_issue", &"<formatter>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    //! Tests for Fluent resource parsing and identifier normalization helpers.
    use super::*;
    use rstest::rstest;
    use unic_langid::langid;

    // =========================================================================
    // default_resources tests
    // =========================================================================

    #[rstest]
    #[case::en_us(langid!("en-US"), EN_US_RESOURCES.as_slice(), "EN_US_RESOURCES", "en-US should return English resources")]
    #[case::en_gb(langid!("en-GB"), EN_US_RESOURCES.as_slice(), "EN_US_RESOURCES", "en-GB should return English resources (language-based matching)")]
    #[case::bare_en(langid!("en"), EN_US_RESOURCES.as_slice(), "EN_US_RESOURCES", "bare 'en' should return English resources")]
    #[case::ja(langid!("ja"), JA_RESOURCES.as_slice(), "JA_RESOURCES", "ja should return Japanese resources")]
    #[case::ja_jp(langid!("ja-JP"), JA_RESOURCES.as_slice(), "JA_RESOURCES", "ja-JP should return Japanese resources (language-based matching)")]
    fn default_resources_returns_expected_catalogue(
        #[case] locale: LanguageIdentifier,
        #[case] expected_resources: &'static [&'static str],
        #[case] resource_name: &str,
        #[case] description: &str,
    ) {
        let resources = default_resources(&locale);
        assert!(
            resources.is_some(),
            "{description}: should return Some for locale {locale}"
        );
        assert!(
            std::ptr::eq(resources.expect("checked above"), expected_resources),
            "{description}: should return {resource_name} for locale {locale}"
        );
    }

    #[test]
    fn default_resources_returns_none_for_unsupported_locale() {
        let locale = langid!("fr-FR");
        let resources = default_resources(&locale);
        assert!(resources.is_none(), "unsupported locale should return None");
    }
}
