//! Unit tests for summary-locale selection diagnostics.
//!
//! These tests drive the generation boundary so the structured `info`, `warn`,
//! and `debug` events are observed from the real selection outcome.

use super::*;
use crate::cli_test_support;
use crate::cli_test_support::PackageRoot;
use crate::generation::build_agent_context_localizer_if_requested;
use crate::locale::parse_locale;
use rstest::{fixture, rstest};

/// Builds an empty package root for one test.
#[fixture]
fn package_root() -> PackageRoot {
    PackageRoot::new()
}

/// Builds a candidate list containing one `en-US` preference.
fn en_us_preferences() -> Vec<SummaryLocaleCandidate> {
    vec![SummaryLocaleCandidate {
        locale: parse_locale("en-US").expect("parse en-US"),
        source: SummaryLocaleSource::EnUs,
    }]
}

/// Builds arguments that request the agent-context format.
fn args_for(locales: &[&str]) -> Args {
    let mut args = cli_test_support::base_args();
    args.format = crate::cli::OutputFormat::AgentContext;
    args.locale = locales.iter().map(|locale| (*locale).to_owned()).collect();
    args
}

/// Selects a localizer through the generation boundary so diagnostics run.
fn select_at_boundary(
    args: &Args,
    package_root: &cli_test_support::PackageRoot,
    metadata_locales: Option<Vec<String>>,
) -> Option<SummaryLocalizer> {
    cli_test_support::ensure_capture_subscriber();
    let selection =
        cli_test_support::selection_with_locales(package_root.path.clone(), metadata_locales);
    build_agent_context_localizer_if_requested(args, &selection)
}
/// A fall-through from the first preference emits the documented warning.
#[rstest]
fn fall_through_from_the_first_preference_warns(package_root: PackageRoot) {
    package_root.add_catalogue("en-US", "demo-about = American English summary.\n");
    let args = args_for(&["fr-FR"]);

    let events = cli_test_support::capture_events(|| {
        let _selected = select_at_boundary(&args, &package_root, None);
    });

    let found = events
        .iter()
        .find(|event| event.message_contains("differs from the first preference"));
    let warning = found.expect("fall-through should warn");
    assert_eq!(
        warning.field("first_preference"),
        "fr-FR",
        "the warning should name the first preference",
    );
    assert_eq!(
        warning.field("chosen"),
        "en-US",
        "the warning should name the chosen locale",
    );
}

/// Reaching the embedded defaults emits the documented warning.
#[rstest]
fn embedded_defaults_warn(package_root: PackageRoot) {
    let args = args_for(&[]);

    let events = cli_test_support::capture_events(|| {
        let _selected = select_at_boundary(&args, &package_root, None);
    });

    assert!(
        events
            .iter()
            .any(|event| event.message_contains("summaries use embedded defaults")),
        "reaching the embedded defaults should warn: {:?}",
        events
            .iter()
            .map(|event| event.field("message"))
            .collect::<Vec<_>>(),
    );
}

/// A malformed catalogue emits the candidate-failure warning naming it.
#[rstest]
fn failed_candidate_warns_with_its_locale(package_root: PackageRoot) {
    package_root.add_catalogue("en-GB", "demo-about = { $unclosed\n");
    let args = args_for(&["en-GB"]);

    let events = cli_test_support::capture_events(|| {
        let _selected = select_at_boundary(&args, &package_root, None);
    });

    let messages = events
        .iter()
        .map(|event| event.field("message").to_owned())
        .collect::<Vec<_>>();
    let found = events
        .iter()
        .find(|event| event.message_contains("candidate failed; trying the next"));
    let warning =
        found.unwrap_or_else(|| panic!("a malformed catalogue should warn: {messages:?}"));
    assert_eq!(
        warning.field("locale"),
        "en-GB",
        "the warning should name the failed candidate",
    );
}

/// Discovery failure is reported, and selection still falls back.
#[rstest]
fn discovery_failure_is_reported_and_selection_continues(package_root: PackageRoot) {
    package_root.write_file("locales", "not a directory");
    let preferences = en_us_preferences();

    let discovered = same_language_summary_locales(&package_root.path, &preferences);

    assert!(
        discovered.is_err(),
        "a regular `locales` file should fail discovery, got {discovered:?}",
    );

    let args = args_for(&[]);
    let mut selected = None;
    let events = cli_test_support::capture_events(|| {
        selected = select_at_boundary(&args, &package_root, None);
    });
    let resolved = selected.expect("embedded defaults should still supply summaries");
    assert_eq!(
        resolved.source,
        SummaryLocaleSource::EmbeddedDefaults,
        "selection should continue to the embedded defaults",
    );
    let found = events
        .iter()
        .find(|event| event.message_contains("locale discovery failed"));
    let warning = found.expect("the discovery failure should warn");
    assert!(
        !warning.field("error").is_empty(),
        "the warning should carry the discovery error",
    );
}

/// An empty locale directory is skipped and reported at debug level.
#[rstest]
fn empty_locale_directory_is_skipped_and_reported(package_root: PackageRoot) {
    package_root.add_locale_dir("en-US");
    let args = args_for(&["en-US"]);
    let selection = cli_test_support::selection_with_locales(package_root.path.clone(), None);

    let outcome = select_summary_localizer(&args, &selection);
    let en_us = parse_locale("en-US").expect("parse en-US");
    assert!(
        outcome.skipped.iter().any(|skipped| {
            skipped.kind == SummaryLocaleSkipKind::NoResources && skipped.candidate.locale == en_us
        }),
        "the empty en-US directory should be recorded as having no resources, got {:?}",
        outcome
            .skipped
            .iter()
            .map(|skipped| (&skipped.candidate.locale, skipped.kind))
            .collect::<Vec<_>>(),
    );

    let events = cli_test_support::capture_events(|| {
        let _selected = select_at_boundary(&args, &package_root, None);
    });
    assert!(
        events
            .iter()
            .any(|event| event.message_contains("no Fluent resources; skipping")),
        "the empty directory should produce the debug skip event",
    );
}
