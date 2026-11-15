//! Compatibility characterization before extracting the reusable runtime.

use super::*;
use fluent_bundle::{FluentResource, concurrent::FluentBundle};
use rstest::rstest;

#[test]
fn reusable_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<FluentLocalizer>();
    assert_send_sync::<NoOpLocalizer>();
    assert_send_sync::<Box<dyn Localizer>>();
    assert_send_sync::<FormattingIssueReporter>();
}

#[test]
fn prebuilt_bundle_replaces_even_invalid_resource_strings() {
    let mut bundle = FluentBundle::new_concurrent(vec![langid!("ja")]);
    bundle
        .add_resource(Arc::new(
            FluentResource::try_new("custom = Bundle value".to_owned()).expect("valid resource"),
        ))
        .expect("unique resource");
    let localizer = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(["not a valid resource"])
        .with_consumer_bundle(bundle)
        .try_build()
        .expect("prebuilt bundle bypasses resource parsing");
    assert_eq!(
        localizer.lookup("custom", None).as_deref(),
        Some("Bundle value")
    );
    assert_eq!(localizer.locale(), Some(&langid!("en-GB")));
}

#[rstest]
#[case::resources(vec!["custom = Bonjour"])]
#[case::empty(Vec::new())]
fn unsupported_locale_fails_before_consumer_build(#[case] resources: Vec<&'static str>) {
    let error = FluentLocalizer::builder(langid!("fr"))
        .with_consumer_resources(resources)
        .try_build()
        .expect_err("defaults do not support French");
    assert!(
        matches!(error, FluentLocalizerError::UnsupportedLocale { locale } if locale == langid!("fr"))
    );
}

#[test]
fn disabling_defaults_allows_an_empty_unsupported_locale() {
    let localizer = FluentLocalizer::builder(langid!("fr"))
        .disable_defaults()
        .try_build()
        .expect("no catalogues need loading");
    assert_eq!(localizer.lookup("cli.about", None), None);
    assert_eq!(localizer.locale(), None);
    assert_eq!(localizer.message("missing", None, "fallback"), "fallback");
}

#[test]
fn regional_english_keeps_requested_locale_and_default_copy() {
    let regional = FluentLocalizer::embedded(langid!("en-GB")).expect("English defaults");
    let american = FluentLocalizer::embedded(langid!("en-US")).expect("English defaults");
    assert_eq!(
        regional.lookup("cli.about", None),
        american.lookup("cli.about", None)
    );
    assert_eq!(regional.locale(), Some(&langid!("en-GB")));
}

#[test]
fn duplicate_messages_produce_registration_errors() {
    let error = FluentLocalizer::builder(langid!("en-US"))
        .with_consumer_resources(["custom = First", "custom = Second"])
        .try_build()
        .expect_err("duplicate registration must fail");
    match error {
        FluentLocalizerError::Registration {
            locale,
            catalogue,
            errors,
        } => {
            assert_eq!(locale, langid!("en-US"));
            assert_eq!(catalogue, FluentBundleSource::Consumer);
            assert_ne!(errors, Vec::new());
        }
        other => panic!("expected registration error, got {other:?}"),
    }
}

#[test]
fn formatting_failure_reports_once_then_uses_defaults() {
    let issues = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = Arc::clone(&issues);
    let localizer = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(["cli.about = { $missing }"])
        .with_error_reporter(Arc::new(move |issue| {
            captured.lock().expect("report mutex").push(issue.clone());
        }))
        .try_build()
        .expect("valid catalogues");
    let defaults = FluentLocalizer::embedded(langid!("en-GB")).expect("English defaults");
    assert_eq!(
        localizer.lookup("cli.about", None),
        defaults.lookup("cli.about", None)
    );
    let reported = issues.lock().expect("report mutex");
    assert_eq!(
        reported.len(),
        1,
        "do not retry the failed pattern in the same bundle"
    );
    let issue = reported.first().expect("one formatting issue");
    assert_eq!(issue.id, "cli.about");
    assert_eq!(issue.locale, langid!("en-GB"));
    assert_eq!(issue.source, FluentBundleSource::Consumer);
    assert_ne!(issue.errors, Vec::new());
}
