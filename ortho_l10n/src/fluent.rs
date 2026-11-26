//! Fluent bundle utilities extracted from the localization module.
//!
//! Keeping parsing, resource registration, and identifier normalization in a
//! dedicated module keeps `mod.rs` concise while retaining cohesion around
//! Fluent-specific concerns.

use fluent_bundle::FluentResource;
use fluent_bundle::concurrent::FluentBundle;
use std::borrow::Cow;
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
    /// Applies Fluent's bidirectional-isolation setting without exposing the bundle.
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

/// Converts runtime dotted message paths to `Fluent`'s hyphen-separated identifier spelling.
///
/// IDs without dots are borrowed unchanged, avoiding allocation on the common lookup path.
pub(super) fn normalize_identifier(id: &str) -> Cow<'_, str> {
    if id.contains('.') {
        Cow::Owned(id.replace('.', super::MESSAGE_ID_SEPARATOR))
    } else {
        Cow::Borrowed(id)
    }
}

/// Normalizes top-level catalogue IDs while retaining message bodies and line boundaries.
pub(super) fn normalize_resource_ids(resource: &str) -> String {
    resource
        .lines()
        .map(normalize_id_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns true when `ch` is accepted by the load-time id normalizer.
///
/// Fluent identifiers are ASCII-only and do not permit `.`. This load-time
/// pre-normalization predicate is intentionally more tolerant so dotted
/// catalogue ids and existing non-ASCII resource ids can be normalized or
/// passed through before Fluent parses them.
fn is_valid_load_time_id_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

/// Validates a load-time resource identifier before pre-normalization.
fn is_valid_load_time_identifier(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_alphabetic() && chars.all(is_valid_load_time_id_char)
}

/// Rewrites only a valid, unindented assignment key; comments and Fluent body lines pass through.
fn normalize_id_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return line.to_owned();
    }

    let Some((left, right)) = line.split_once('=') else {
        return line.to_owned();
    };

    // Only normalize top-level identifiers. Indented lines belong to message
    // bodies, attributes, or variants and must be left untouched.
    if left.chars().next().is_some_and(char::is_whitespace) {
        return line.to_owned();
    }

    let id_segment = left.trim_end();
    if !is_valid_load_time_identifier(id_segment) {
        return line.to_owned();
    }

    let normalized_id = normalize_identifier(id_segment).into_owned();
    let trailing_ws = left.strip_prefix(id_segment).unwrap_or_default();

    let mut rebuilt = String::with_capacity(line.len());
    rebuilt.push_str(&normalized_id);
    rebuilt.push_str(trailing_ws);
    rebuilt.push('=');
    rebuilt.push_str(right);
    rebuilt
}

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

    /// Chooses whether formatted values carry Unicode bidirectional isolates.
    ///
    /// Fluent defaults to `true` and wraps every interpolated value in
    /// U+2068 FIRST STRONG ISOLATE and U+2069 POP DIRECTIONAL ISOLATE. The
    /// marks keep right-to-left values readable inside left-to-right text, but
    /// many terminals, fonts, and log viewers do not display them, and they
    /// break `grep`, string comparison, and snapshot tests on diagnostics.
    ///
    /// An explicit setting is applied once during [`try_build`] to both
    /// bundles this builder owns: the embedded defaults and the consumer
    /// bundle. That includes a bundle supplied through
    /// [`with_consumer_bundle`]. The last call wins, and the order relative to
    /// other setters does not matter.
    ///
    /// When this method is not called, each bundle keeps the setting it was
    /// created with: Fluent's `true` default for bundles built from resources,
    /// and whatever the caller chose for a supplied bundle. Disabling
    /// isolation is not general bidirectional sanitization. Turn it off only
    /// when the rendered text is known to be left-to-right, as is usual for
    /// terminal diagnostics.
    ///
    /// [`with_consumer_bundle`]: FluentLocalizerBuilder::with_consumer_bundle
    /// [`try_build`]: FluentLocalizerBuilder::try_build
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_l10n::{FluentLocalizer, FluentValue, LocalizationArgs, Localizer, langid};
    ///
    /// let localizer = FluentLocalizer::builder(langid!("en-GB"))
    ///     .with_consumer_resources(["welcome = Hello, { $name }!"])
    ///     .with_use_isolating(false)
    ///     .try_build()?;
    /// let mut args = LocalizationArgs::new();
    /// args.insert("name", FluentValue::from("Ada"));
    /// assert_eq!(
    ///     localizer.lookup("welcome", Some(&args)).as_deref(),
    ///     Some("Hello, Ada!"),
    /// );
    /// # Ok::<(), ortho_l10n::FluentLocalizerError>(())
    /// ```
    #[must_use]
    pub fn with_use_isolating(mut self, use_isolating: bool) -> Self {
        self.use_isolating = Some(use_isolating);
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
        if let Some(use_isolating) = self.use_isolating {
            for bundle in [defaults.as_mut(), consumer.as_mut()].into_iter().flatten() {
                bundle.set_use_isolating(use_isolating);
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
#[path = "fluent_tests.rs"]
mod tests;
