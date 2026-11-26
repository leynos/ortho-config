//! Isolation-output tests for [`FluentLocalizerBuilder::with_use_isolating`].
//!
//! Fluent wraps interpolated values in Unicode bidirectional isolates by
//! default. These tests pin the exact rendered text for the unspecified,
//! explicitly enabled, and explicitly disabled cases across the embedded
//! defaults, resource-built consumer bundles, and supplied prebuilt bundles.

use super::*;
use fluent_bundle::{FluentResource, concurrent::FluentBundle};
use rstest::rstest;
use std::sync::{Arc, Mutex};

/// Greeting rendered with Fluent's isolate marks around the interpolated name.
const ISOLATED_GREETING: &str = "Hello, \u{2068}Ada\u{2069}!";
/// Greeting rendered without any isolate marks.
const PLAIN_GREETING: &str = "Hello, Ada!";
/// Embedded usage copy with the binary name wrapped in isolate marks.
const ISOLATED_USAGE: &str = "Usage: \u{2068}demo\u{2069} [OPTIONS] <COMMAND>";
/// Embedded usage copy without isolate marks.
const PLAIN_USAGE: &str = "Usage: demo [OPTIONS] <COMMAND>";

/// Lookup arguments for the greeting catalogue entry.
fn greeting_args() -> LocalizationArgs<'static> {
    let mut args = LocalizationArgs::new();
    args.insert("name", FluentValue::from("Ada"));
    args
}

/// Lookup arguments for the embedded usage entry.
fn usage_args() -> LocalizationArgs<'static> {
    let mut args = LocalizationArgs::new();
    args.insert("binary", FluentValue::from("demo"));
    args
}

/// Applies [`FluentLocalizerBuilder::with_use_isolating`] only when a setting
/// was supplied, so `None` exercises the unspecified path.
fn with_optional_isolation(
    builder: FluentLocalizerBuilder,
    setting: Option<bool>,
) -> FluentLocalizerBuilder {
    match setting {
        Some(value) => builder.with_use_isolating(value),
        None => builder,
    }
}

/// Renders the consumer greeting from a catalogue built with the given setting.
fn render_consumer_greeting(setting: Option<bool>) -> String {
    let builder = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(["welcome = Hello, { $name }!"]);
    let localizer = with_optional_isolation(builder, setting)
        .try_build()
        .expect("consumer catalogue should build");
    localizer
        .lookup("welcome", Some(&greeting_args()))
        .expect("welcome message should resolve")
}

/// Builds a concurrent bundle whose interpolation setting is chosen up front.
fn prebuilt_bundle(use_isolating: bool) -> FluentBundle<Arc<FluentResource>> {
    let mut bundle = FluentBundle::new_concurrent(vec![langid!("en-GB")]);
    bundle.set_use_isolating(use_isolating);
    bundle
        .add_resource(Arc::new(
            FluentResource::try_new("welcome = Hello, { $name }!".to_owned())
                .expect("greeting resource should parse"),
        ))
        .expect("greeting resource id should be unique");
    bundle
}

#[rstest]
#[case::unspecified(None, ISOLATED_GREETING)]
#[case::explicitly_enabled(Some(true), ISOLATED_GREETING)]
#[case::explicitly_disabled(Some(false), PLAIN_GREETING)]
fn consumer_bundle_follows_isolation_setting(
    #[case] setting: Option<bool>,
    #[case] expected: &str,
) {
    assert_eq!(
        render_consumer_greeting(setting),
        expected,
        "resource-built consumer bundle should honour {setting:?}"
    );
}

#[rstest]
#[case::unspecified(None, ISOLATED_USAGE)]
#[case::explicitly_enabled(Some(true), ISOLATED_USAGE)]
#[case::explicitly_disabled(Some(false), PLAIN_USAGE)]
fn embedded_defaults_follow_isolation_setting(
    #[case] setting: Option<bool>,
    #[case] expected: &str,
) {
    let builder = FluentLocalizer::builder(langid!("en-US"));
    let localizer = with_optional_isolation(builder, setting)
        .try_build()
        .expect("embedded catalogue should build");
    let rendered = localizer
        .lookup("cli.usage", Some(&usage_args()))
        .expect("embedded usage message should resolve");
    assert_eq!(
        rendered, expected,
        "embedded defaults should honour {setting:?}"
    );
}

#[rstest]
#[case::unspecified_keeps_supplied_off(false, None, PLAIN_GREETING)]
#[case::unspecified_keeps_supplied_on(true, None, ISOLATED_GREETING)]
#[case::explicit_off_overrides_supplied_on(true, Some(false), PLAIN_GREETING)]
#[case::explicit_on_overrides_supplied_off(false, Some(true), ISOLATED_GREETING)]
fn supplied_bundle_setting_and_explicit_override(
    #[case] supplied_use_isolating: bool,
    #[case] setting: Option<bool>,
    #[case] expected: &str,
) {
    let builder = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_bundle(prebuilt_bundle(supplied_use_isolating));
    let localizer = with_optional_isolation(builder, setting)
        .try_build()
        .expect("supplied bundle should build");
    let rendered = localizer
        .lookup("welcome", Some(&greeting_args()))
        .expect("supplied greeting should resolve");
    assert_eq!(
        rendered, expected,
        "a supplied bundle that isolates={supplied_use_isolating} with setting {setting:?}"
    );
}

#[rstest]
#[case::enable_after_disable(false, true, ISOLATED_GREETING)]
#[case::disable_after_enable(true, false, PLAIN_GREETING)]
#[case::repeat_disable(false, false, PLAIN_GREETING)]
#[case::repeat_enable(true, true, ISOLATED_GREETING)]
fn last_isolation_setting_wins(#[case] first: bool, #[case] last: bool, #[case] expected: &str) {
    let builder = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(["welcome = Hello, { $name }!"])
        .with_use_isolating(first)
        .with_use_isolating(last);
    let localizer = builder
        .try_build()
        .expect("consumer catalogue should build");
    let rendered = localizer
        .lookup("welcome", Some(&greeting_args()))
        .expect("welcome message should resolve");
    assert_eq!(
        rendered, expected,
        "the final setting {last} should win after an earlier {first}"
    );
}

#[test]
fn isolation_setting_is_independent_of_resource_setter_order() {
    let setting_first = FluentLocalizer::builder(langid!("en-GB"))
        .with_use_isolating(false)
        .with_consumer_resources(["welcome = Hello, { $name }!"])
        .try_build()
        .expect("consumer catalogue should build");
    let setting_last = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(["welcome = Hello, { $name }!"])
        .with_use_isolating(false)
        .try_build()
        .expect("consumer catalogue should build");

    let render = |localizer: &FluentLocalizer| {
        localizer
            .lookup("welcome", Some(&greeting_args()))
            .expect("welcome message should resolve")
    };
    assert_eq!(
        render(&setting_first),
        PLAIN_GREETING,
        "calling with_use_isolating before the resources should still apply"
    );
    assert_eq!(
        render(&setting_last),
        PLAIN_GREETING,
        "calling with_use_isolating after the resources should still apply"
    );
}

#[test]
fn missing_messages_still_use_the_fallback_when_isolation_is_disabled() {
    let localizer = FluentLocalizer::builder(langid!("en-GB"))
        .with_use_isolating(false)
        .try_build()
        .expect("embedded catalogue should build");
    assert_eq!(
        localizer.message("absent.message", None, "fallback"),
        "fallback",
        "a missing message should return the unchanged fallback"
    );
}

#[test]
fn formatting_failure_reports_once_and_falls_back_without_isolates() {
    let issues = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&issues);
    let localizer = FluentLocalizer::builder(langid!("en-GB"))
        .with_consumer_resources(["cli.usage = Usage { $binary } { $missing }"])
        .with_use_isolating(false)
        .with_error_reporter(Arc::new(move |issue: &FormattingIssue| {
            captured
                .lock()
                .expect("issue log mutex should not be poisoned")
                .push(issue.id.clone());
        }))
        .try_build()
        .expect("catalogues should build");

    let rendered = localizer
        .lookup("cli.usage", Some(&usage_args()))
        .expect("embedded defaults should provide the fallback copy");
    assert_eq!(
        rendered, PLAIN_USAGE,
        "the fallback bundle should also render without isolates"
    );
    assert_eq!(
        *issues
            .lock()
            .expect("issue log mutex should not be poisoned"),
        vec!["cli.usage".to_owned()],
        "the failed consumer pattern should be reported exactly once"
    );
}

#[test]
fn formatting_failure_without_defaults_returns_none_when_isolation_is_disabled() {
    let issues = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&issues);
    let localizer = FluentLocalizer::builder(langid!("fr"))
        .disable_defaults()
        .with_consumer_resources(["welcome = Hello, { $missing }!"])
        .with_use_isolating(false)
        .with_error_reporter(Arc::new(move |issue: &FormattingIssue| {
            captured
                .lock()
                .expect("issue log mutex should not be poisoned")
                .push(issue.id.clone());
        }))
        .try_build()
        .expect("consumer-only catalogue should build");

    assert_eq!(
        localizer.lookup("welcome", None),
        None,
        "a formatting failure without defaults should resolve to None"
    );
    assert_eq!(
        *issues
            .lock()
            .expect("issue log mutex should not be poisoned"),
        vec!["welcome".to_owned()],
        "the failed consumer pattern should still be reported"
    );
}
