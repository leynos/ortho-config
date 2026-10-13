//! Demo localiser showcasing how applications can layer Fluent resources over
//! the embedded defaults provided by `ortho_config`.
//!
//! The example embeds English and Japanese catalogues and builds a
//! [`FluentLocalizer`] so the CLI copy is translated through the same abstraction
//! used by consumer binaries. Should localisation setup fail, the localiser
//! falls back to [`NoOpLocalizer`] and retains the stock `clap` strings.
//!
//! Locale selection is performed by inspecting environment variables (`LANG`,
//! `LC_ALL`, `LC_MESSAGES`) in priority order, falling back to `en-US` when no
//! supported locale is detected.

use ortho_config::{
    FluentLocalizer, FluentLocalizerBuilder, FluentLocalizerError, LanguageIdentifier,
    LocalizationArgs, Localizer, NoOpLocalizer, OrthoConfigLocalization, langid,
};
use tracing::warn;

use crate::cli::CommandLine;

/// Base identifier for the hello world CLI catalogue (derive-derived).
pub const CLI_BASE_MESSAGE_ID: &str = CommandLine::LOCALIZATION_BASE;
/// Identifier used for the short CLI description (derive-derived).
pub const CLI_ABOUT_MESSAGE_ID: &str = CommandLine::ABOUT_ID;
/// Identifier used for the long CLI description (derive-derived).
pub const CLI_LONG_ABOUT_MESSAGE_ID: &str = CommandLine::LONG_ABOUT_ID;
/// Identifier for the usage string presented on `--help` (derive-derived).
pub const CLI_USAGE_MESSAGE_ID: &str = CommandLine::USAGE_ID;

const HELLO_WORLD_EN_US: &str = include_str!("../locales/en-US/messages.ftl");
const HELLO_WORLD_JA: &str = include_str!("../locales/ja/messages.ftl");

/// Environment variable names checked for locale preference, in priority order.
const LOCALE_ENV_VARS: [&str; 3] = ["LC_ALL", "LC_MESSAGES", "LANG"];

/// Localiser that layers the example's Fluent catalogue over the embedded defaults.
pub struct DemoLocalizer {
    inner: Option<FluentLocalizer>,
    noop: NoOpLocalizer,
}

impl Default for DemoLocalizer {
    fn default() -> Self {
        Self::new()
    }
}

impl DemoLocalizer {
    /// Builds the demo localiser using the detected system locale,
    /// falling back to [`NoOpLocalizer`] if Fluent setup fails.
    ///
    /// # Examples
    /// ```rust
    /// use clap::CommandFactory;
    /// use hello_world::cli::{CommandLine, LocalizeCmd};
    /// use hello_world::localizer::DemoLocalizer;
    /// use ortho_config::OrthoConfigLocalization;
    ///
    /// let localizer = DemoLocalizer::new();
    /// let command = CommandLine::command()
    ///     .with_base(CommandLine::LOCALIZATION_BASE)
    ///     .localize(&localizer);
    /// assert!(command.get_about().is_some());
    /// ```
    ///
    #[must_use]
    pub fn new() -> Self {
        Self::for_locale(detect_locale())
    }

    /// Builds the demo localiser for a specific locale,
    /// falling back to [`NoOpLocalizer`] if Fluent setup fails.
    #[must_use]
    pub fn for_locale(locale: LanguageIdentifier) -> Self {
        match Self::try_for_locale(locale) {
            Ok(localiser) => localiser,
            Err(error) => {
                warn!(?error, "falling back to no-op localiser");
                Self {
                    inner: None,
                    noop: NoOpLocalizer::new(),
                }
            }
        }
    }

    /// Attempts to construct the demo localiser for a specific locale.
    ///
    /// # Errors
    ///
    /// Returns an error when the embedded Fluent catalogue cannot be parsed
    /// or registered for the requested locale.
    pub fn try_for_locale(locale: LanguageIdentifier) -> Result<Self, FluentLocalizerError> {
        let resources = consumer_resources_for(&locale);
        Ok(Self {
            inner: Some(
                FluentLocalizerBuilder::new(locale)
                    .with_consumer_resources(resources)
                    .try_build()?,
            ),
            noop: NoOpLocalizer::new(),
        })
    }

    /// Attempts to construct the demo localiser for English (en-US).
    ///
    /// # Errors
    ///
    /// Returns an error when the embedded Fluent catalogue cannot be parsed
    /// or registered.
    pub fn try_new() -> Result<Self, FluentLocalizerError> {
        Self::try_for_locale(langid!("en-US"))
    }

    /// Returns the underlying [`FluentLocalizer`] when available so consumers
    /// can reuse the same catalogue instance.
    #[must_use]
    pub const fn fluent(&self) -> Option<&FluentLocalizer> {
        self.inner.as_ref()
    }

    /// Provides a no-op localiser for callers that do not ship translations yet.
    #[must_use]
    pub const fn noop() -> NoOpLocalizer {
        NoOpLocalizer::new()
    }
}

impl Localizer for DemoLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        self.inner.as_ref().map_or_else(
            || self.noop.lookup(id, args),
            |fluent| fluent.lookup(id, args),
        )
    }
}

impl std::fmt::Debug for DemoLocalizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner = if self.inner.is_some() {
            "fluent"
        } else {
            "noop"
        };

        f.debug_struct("DemoLocalizer")
            .field("inner", &inner)
            .field("noop", &self.noop)
            .finish()
    }
}

/// Returns the consumer resources appropriate for the given locale.
fn consumer_resources_for(locale: &LanguageIdentifier) -> Vec<&'static str> {
    if is_japanese(locale) {
        vec![HELLO_WORLD_JA]
    } else {
        vec![HELLO_WORLD_EN_US]
    }
}

/// Checks whether the locale represents a Japanese variant.
fn is_japanese(locale: &LanguageIdentifier) -> bool {
    locale.language.as_str() == "ja"
}

/// Detects the preferred locale from environment variables.
///
/// Inspects `LC_ALL`, `LC_MESSAGES`, and `LANG` in priority order, parsing
/// POSIX-style locale strings (e.g., `ja_JP.UTF-8`) into language identifiers.
/// Falls back to `en-US` when no supported locale is detected.
#[must_use]
pub fn detect_locale() -> LanguageIdentifier {
    for var_name in LOCALE_ENV_VARS {
        if let Some(locale) = parse_locale_from_env(var_name) {
            return locale;
        }
    }
    langid!("en-US")
}

/// Parses a locale from the named environment variable.
fn parse_locale_from_env(var_name: &str) -> Option<LanguageIdentifier> {
    let value = std::env::var(var_name).ok()?;
    parse_posix_locale(&value)
}

/// Parses a POSIX locale string into a `LanguageIdentifier`.
///
/// Handles formats like `ja_JP.UTF-8` by stripping the encoding suffix and
/// converting underscores to hyphens for BCP 47 compatibility. Special values
/// `C` and `POSIX` are treated as English.
fn parse_posix_locale(value: &str) -> Option<LanguageIdentifier> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Handle "C" and "POSIX" special cases
    if trimmed == "C" || trimmed == "POSIX" {
        return Some(langid!("en-US"));
    }

    // Strip encoding suffix (e.g., ".UTF-8")
    let without_encoding = trimmed.split('.').next()?;

    // Replace underscore with hyphen for BCP 47 compatibility
    let normalized = without_encoding.replace('_', "-");

    normalized.parse().ok()
}

#[cfg(test)]
mod tests {
    //! Unit tests for the demo localizer.

    use super::*;
    use rstest::rstest;
    use std::collections::HashMap;

    #[test]
    fn demo_localiser_returns_copy() {
        let localiser = DemoLocalizer::try_new().expect("demo localiser should build");
        let about = localiser
            .lookup(CLI_ABOUT_MESSAGE_ID, None)
            .expect("demo copy should exist");
        assert!(about.contains("greeting"));
    }

    #[test]
    fn demo_localiser_formats_long_description() {
        let localiser = DemoLocalizer::try_new().expect("demo localiser should build");
        let mut args: LocalizationArgs<'_> = HashMap::new();
        args.insert("binary", "hello-world".into());
        let long_about = localiser
            .lookup(CLI_LONG_ABOUT_MESSAGE_ID, Some(&args))
            .expect("long description should exist");
        assert!(long_about.contains("hello-world"));
    }

    #[test]
    fn fluent_returns_inner_reference() {
        let localiser = DemoLocalizer::try_new().expect("demo localiser should build");
        let inner_ptr = localiser
            .inner
            .as_ref()
            .map(std::ptr::from_ref::<FluentLocalizer>)
            .expect("expected fluent inner");

        let exposed = localiser.fluent().expect("fluent inner should be exposed");
        assert!(std::ptr::eq(inner_ptr, exposed));
    }

    #[test]
    fn fluent_is_none_when_noop() {
        let localiser = DemoLocalizer {
            inner: None,
            noop: NoOpLocalizer::new(),
        };

        assert!(localiser.fluent().is_none());
    }

    /// Asserts that the localiser for the given locale translates clap error
    /// messages containing the expected substring.
    fn assert_clap_error_translation(locale: LanguageIdentifier, expected_substring: &str) {
        let locale_str = locale.to_string();
        let localiser = match DemoLocalizer::try_for_locale(locale) {
            Ok(localiser) => localiser,
            Err(e) => panic!("localiser for {locale_str} should build: {e}"),
        };

        let mut args: LocalizationArgs<'_> = HashMap::new();
        args.insert("valid_subcommands", "greet, take-leave".into());

        let Some(message) = localiser.lookup("clap-error-missing-subcommand", Some(&args)) else {
            panic!("catalogue for {locale_str} should include clap error copy");
        };

        assert!(
            message.contains(expected_substring),
            "expected '{expected_substring}' in clap error for {locale_str}, got: {message}"
        );
    }

    /// Confirms each bundled catalogue translates the clap error copy.
    #[rstest]
    #[case::english(langid!("en-US"), "Pick a workflow")]
    #[case::japanese(langid!("ja"), "ワークフロー")]
    fn localiser_translates_clap_errors(
        #[case] locale: LanguageIdentifier,
        #[case] expected_substring: &str,
    ) {
        assert_clap_error_translation(locale, expected_substring);
    }

    #[test]
    fn japanese_localiser_returns_japanese_copy() {
        let localiser =
            DemoLocalizer::try_for_locale(langid!("ja")).expect("Japanese localiser should build");
        let about = localiser
            .lookup(CLI_ABOUT_MESSAGE_ID, None)
            .expect("Japanese demo copy should exist");
        assert!(about.contains("挨拶"));
    }

    /// Checks POSIX locale parsing across encodings, special values, and blanks.
    #[rstest]
    #[case::utf8_suffix("ja_JP.UTF-8", Some(langid!("ja-JP")))]
    #[case::bare_language("en", Some(langid!("en")))]
    #[case::c_locale("C", Some(langid!("en-US")))]
    #[case::posix_locale("POSIX", Some(langid!("en-US")))]
    #[case::empty_string("", None)]
    #[case::whitespace_only("   ", None)]
    fn parse_posix_locale_maps_input_to_locale(
        #[case] input: &str,
        #[case] expected: Option<LanguageIdentifier>,
    ) {
        assert_eq!(parse_posix_locale(input), expected);
    }
}
