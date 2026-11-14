//! Rendering coverage for the published clap error translation matrix.
//!
//! The matrix suite proves which identifiers exist and which kinds are
//! display-only. This suite proves the other half of the contract: every
//! `Translated` identifier has a real embedded en-US message rather than
//! relying on fallback text, that message renders without Fluent errors for
//! the arguments clap supplies, consumer catalogues can override it, and the
//! stock diagnostic survives whenever translation is unavailable.

use clap::error::Error as ClapError;
use ortho_config::{
    CLAP_ERROR_IDS, ClapErrorTranslation, FluentLocalizer, FormattingIssueReporter,
    LocalizationArgs, Localizer, langid, localize_clap_error,
};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard};

/// Placeholder values for every translated identifier, mirroring the context
/// clap supplies for that kind.
///
/// A message that starts requiring a placeholder clap cannot provide fails in
/// these tests rather than at runtime. The four identifiers predating the
/// matrix keep their historical spelling and appear here unchanged.
const REPRESENTATIVE_ARGUMENTS: &[(&str, &[(&str, &str)])] = &[
    ("clap-error-missing-argument", &[("argument", "--config")]),
    ("clap-error-unknown-argument", &[("argument", "--unknown")]),
    (
        "clap-error-invalid-value",
        &[("argument", "--speed"), ("value", "fastest")],
    ),
    (
        "clap-error-missing-subcommand",
        &[("valid_subcommands", "greet, take-leave")],
    ),
    ("clap-error-invalid-subcommand", &[("subcommand", "confi")]),
    ("clap-error-no-equals", &[("argument", "--colour")]),
    (
        "clap-error-value-validation",
        &[("argument", "--count"), ("value", "abc")],
    ),
    ("clap-error-too-many-values", &[("argument", "ITEM")]),
    (
        "clap-error-too-few-values",
        &[("argument", "--opt"), ("min", "3"), ("actual", "2")],
    ),
    (
        "clap-error-wrong-number-of-values",
        &[("argument", "--opt"), ("expected", "2"), ("actual", "1")],
    ),
    ("clap-error-argument-conflict", &[("argument", "--debug")]),
    ("clap-error-invalid-utf8", &[]),
    ("clap-error-io", &[]),
    ("clap-error-format", &[]),
];

/// Collects the identifiers Fluent failed to format, in observation order.
#[derive(Default)]
struct FormattingIssues {
    /// Identifiers reported by the localizer under test.
    ids: Mutex<Vec<String>>,
}

impl FormattingIssues {
    /// Creates an empty collector.
    fn new() -> Self {
        Self::default()
    }

    /// Returns the collected identifiers, recovering from a poisoned lock.
    fn ids(&self) -> MutexGuard<'_, Vec<String>> {
        self.ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Builds an en-US localizer that records formatting failures.
///
/// The catalogue is compiled into this crate, so failing to register it is an
/// environment fault rather than a translation under test. The panic names the
/// invariant instead of letting a fallback masquerade as a passing case.
fn reporting_localizer(issues: &Arc<FormattingIssues>) -> FluentLocalizer {
    let collected = Arc::clone(issues);
    let reporter: FormattingIssueReporter = Arc::new(move |issue| {
        collected.ids().push(issue.id.clone());
    });
    match FluentLocalizer::builder(langid!("en-US"))
        .with_error_reporter(reporter)
        .try_build()
    {
        Ok(localizer) => localizer,
        Err(error) => panic!("embedded en-US catalogue should parse and register: {error}"),
    }
}

/// Yields the identifiers the matrix marks as translated.
fn translated_identifiers() -> impl Iterator<Item = &'static str> {
    CLAP_ERROR_IDS
        .iter()
        .filter_map(|(_, outcome)| match outcome {
            ClapErrorTranslation::Translated(identifier) => Some(*identifier),
            // `ClapErrorTranslation` is `#[non_exhaustive]`; a future outcome is
            // not translated by this release's tests.
            _ => None,
        })
}

/// Builds lookup arguments from static placeholder pairs.
fn args_from<'a>(pairs: &[(&'a str, &'a str)]) -> LocalizationArgs<'a> {
    pairs
        .iter()
        .map(|(key, value)| (*key, (*value).into()))
        .collect()
}

/// Asserts every translated identifier has representative arguments.
#[test]
fn representative_arguments_cover_every_translated_identifier() {
    let translated: BTreeSet<&str> = translated_identifiers().collect();
    let covered: BTreeSet<&str> = REPRESENTATIVE_ARGUMENTS
        .iter()
        .map(|(identifier, _)| *identifier)
        .collect();

    assert_eq!(
        covered, translated,
        "representative argument table must match the matrix exactly"
    );
    assert_eq!(
        covered.len(),
        REPRESENTATIVE_ARGUMENTS.len(),
        "representative argument table must not list an identifier twice"
    );
}

/// Asserts each translated identifier resolves to a renderable message.
#[test]
fn every_translated_identifier_renders_without_formatting_issues() {
    let issues = Arc::new(FormattingIssues::new());
    let localizer = reporting_localizer(&issues);

    for (identifier, pairs) in REPRESENTATIVE_ARGUMENTS {
        let args = args_from(pairs);
        let rendered = localizer
            .lookup(identifier, Some(&args))
            .unwrap_or_else(|| panic!("no embedded en-US message for {identifier}"));

        assert!(
            !rendered.trim().is_empty(),
            "translation for {identifier} should not be empty"
        );
        assert!(
            !rendered.contains('{'),
            "translation for {identifier} left a placeholder unresolved: {rendered}"
        );
    }

    let failures = issues.ids();
    assert!(failures.is_empty(), "formatting issues: {failures:?}");
}

/// Asserts a consumer catalogue wins over the embedded one.
#[test]
fn consumer_resources_override_an_embedded_identifier() {
    let localizer = FluentLocalizer::with_en_us_defaults([
        "clap-error-no-equals = Consumer copy for { $argument }",
    ])
    .expect("consumer overlay should parse and register");
    let args = args_from(&[("argument", "--colour")]);

    let rendered = localizer.message("clap-error-no-equals", Some(&args), "fallback");
    assert!(
        rendered.starts_with("Consumer copy for") && rendered.contains("--colour"),
        "consumer override should win over the embedded message, got: {rendered}"
    );
}

/// Asserts a partial consumer catalogue still reaches the embedded message.
#[test]
fn incomplete_consumer_catalogue_retains_the_embedded_translation() {
    let localizer = FluentLocalizer::with_en_us_defaults(["cli-about = Consumer about text"])
        .expect("consumer overlay should parse and register");
    let args = args_from(&[("argument", "--count"), ("value", "abc")]);

    let rendered = localizer.message("clap-error-value-validation", Some(&args), "fallback");
    assert_ne!(
        rendered, "fallback",
        "embedded en-US message should resolve"
    );
    assert!(
        rendered.contains("was rejected by the value parser"),
        "expected the embedded en-US translation, got: {rendered}"
    );
}

/// Asserts a missing placeholder keeps the original diagnostic.
///
/// A raw `MissingSubcommand` error carries no valid-subcommand context, so the
/// Fluent pattern cannot format. The caller must see clap's own message rather
/// than an unresolved placeholder.
#[test]
fn missing_placeholder_context_keeps_the_original_diagnostic() {
    let issues = Arc::new(FormattingIssues::new());
    let localizer = reporting_localizer(&issues);
    let stock = ClapError::raw(clap::error::ErrorKind::MissingSubcommand, "stock body");

    let rendered = localize_clap_error(stock, &localizer).to_string();

    assert_eq!(
        rendered, "error: stock body",
        "a missing placeholder must keep clap's own diagnostic"
    );
    let failures = issues.ids();
    assert_eq!(
        failures.len(),
        1,
        "the catalogue gap should be reported once"
    );
    assert_eq!(
        failures.first().map(String::as_str),
        Some("clap-error-missing-subcommand"),
        "the report should name the translation that could not be formatted"
    );
}

// Real-parse and snapshot coverage lives in a shared module below `support/`
// so Cargo does not also discover it as a standalone integration test, and
// this file stays within the repository's 400-line limit.
#[path = "support/clap_error_rendering.rs"]
mod rendering;
