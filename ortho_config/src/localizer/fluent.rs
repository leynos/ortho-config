//! Fluent bundle utilities extracted from the localisation module.
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

pub(super) struct BundleWithLocale {
    pub(super) locale: LanguageIdentifier,
    pub(super) bundle: FluentBundle<Arc<FluentResource>>,
    pub(super) kind: FluentBundleSource,
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

const EN_US_CATALOGUE: &str = include_str!("../../locales/en-US/messages.ftl");
const JA_CATALOGUE: &str = include_str!("../../locales/ja/messages.ftl");

static EN_US_RESOURCES: [&str; 1] = [EN_US_CATALOGUE];
static JA_RESOURCES: [&str; 1] = [JA_CATALOGUE];

pub(super) fn default_resources(locale: &LanguageIdentifier) -> Option<&'static [&'static str]> {
    match locale.language.as_str() {
        "en" => Some(&EN_US_RESOURCES),
        "ja" => Some(&JA_RESOURCES),
        _ => None,
    }
}

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

pub(super) fn normalize_identifier(id: &str) -> Cow<'_, str> {
    if id.contains('.') {
        Cow::Owned(id.replace('.', "-"))
    } else {
        Cow::Borrowed(id)
    }
}

pub(super) fn normalize_resource_ids(resource: &str) -> String {
    resource
        .lines()
        .map(normalize_id_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns true when `ch` is valid in a Fluent message identifier.
///
/// Fluent identifiers permit Unicode letters and digits alongside `-`, `_`,
/// and `.`. This matches the grammar used by Fluent and avoids excluding
/// non-ASCII locales. See <https://projectfluent.org/fluent/guide/grammar.html>.
fn is_valid_fluent_id_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

/// Validates a Fluent identifier, ensuring the first character is alphabetic
/// and all subsequent characters are permitted by `is_valid_fluent_id_char`.
fn is_valid_fluent_identifier(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_alphabetic() && chars.all(is_valid_fluent_id_char)
}

fn normalize_id_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return line.to_owned();
    }

    let Some((left, right)) = line.split_once('=') else {
        return line.to_owned();
    };

    // Only normalise top-level identifiers. Indented lines belong to message
    // bodies, attributes, or variants and must be left untouched.
    if left.chars().next().is_some_and(char::is_whitespace) {
        return line.to_owned();
    }

    let id_segment = left.trim_end();
    if !is_valid_fluent_identifier(id_segment) {
        return line.to_owned();
    }

    let normalised_id = normalize_identifier(id_segment).into_owned();
    let trailing_ws = left.strip_prefix(id_segment).unwrap_or_default();

    let mut rebuilt = String::with_capacity(line.len());
    rebuilt.push_str(&normalised_id);
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

    /// Builds the [`FluentLocalizer`], validating both default and consumer bundles.
    ///
    /// # Errors
    ///
    /// Returns [`FluentLocalizerError`] if any catalogue fails to parse or
    /// registers conflicting identifiers.
    pub fn try_build(self) -> Result<FluentLocalizer, FluentLocalizerError> {
        let defaults = if self.use_defaults {
            Some(bundle_from_resources(
                &self.locale,
                default_resources(&self.locale).ok_or_else(|| {
                    FluentLocalizerError::UnsupportedLocale {
                        locale: self.locale.clone(),
                    }
                })?,
                FluentBundleSource::Default,
            )?)
        } else {
            None
        };

        let consumer = if let Some(bundle) = self.consumer_bundle {
            Some(bundle)
        } else if self.consumer_resources.is_empty() {
            None
        } else {
            Some(bundle_from_resources(
                &self.locale,
                &self.consumer_resources,
                FluentBundleSource::Consumer,
            )?)
        };

        Ok(FluentLocalizer {
            consumer,
            defaults,
            report_issue: self.report_issue,
        })
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

    // =========================================================================
    // Identifier normalization tests
    // =========================================================================

    #[test]
    fn normalises_top_level_dotted_ids() {
        let resource = "cli.about = About text";
        let normalised = normalize_resource_ids(resource);
        assert_eq!(normalised, "cli-about = About text");
    }

    #[test]
    fn leaves_indented_lines_with_equals_untouched() {
        let resource = "message =\n    section.title = Title line";
        let normalised = normalize_resource_ids(resource);
        assert!(normalised.contains("    section.title = Title line"));
    }

    #[test]
    fn leaves_terms_and_attributes_unchanged() {
        let resource = "-term.id = Term value\nmessage = Value\n    .label = Label";
        let normalised = normalize_resource_ids(resource);
        assert!(normalised.contains("-term.id = Term value"));
        assert!(normalised.contains("    .label = Label"));
    }

    #[test]
    fn normalises_unicode_identifiers() {
        let resource = "ключ.значение = Привет";
        let normalised = normalize_resource_ids(resource);
        assert!(normalised.starts_with("ключ-значение = Привет"));
    }
}
