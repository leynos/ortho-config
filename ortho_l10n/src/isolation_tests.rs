//! Exact-output tests for the builder's `Fluent` isolation override.
//!
//! `Fluent` wraps interpolated values in U+2068 FIRST STRONG ISOLATE and
//! U+2069 POP DIRECTIONAL ISOLATE unless a bundle opts out. These tests assert
//! the rendered text character for character, so an accidental reintroduction
//! of the marks—or an accidental removal of the default ones—fails visibly
//! rather than passing a stripped-substring check.

use super::*;
use fluent_bundle::{FluentResource, concurrent::FluentBundle};
use rstest::rstest;
use std::sync::Arc;

/// Rendered `welcome` message with `Ada` interpolated while isolation is on.
const ISOLATED_WELCOME: &str = "Hello, \u{2068}Ada\u{2069}!";
/// Rendered `welcome` message with `Ada` interpolated and isolation disabled.
const PLAIN_WELCOME: &str = "Hello, Ada!";
/// Consumer resource shared by the exact-output cases.
const WELCOME_RESOURCE: &str = "welcome = Hello, { $name }!";

/// Builds lookup arguments naming the person greeted by `welcome`.
fn welcome_args() -> LocalizationArgs<'static> {
    let mut args = LocalizationArgs::new();
    args.insert("name", FluentValue::from("Ada"));
    args
}

/// Renders `welcome`, requiring the catalogue to supply it.
fn render_welcome(localizer: &FluentLocalizer) -> String {
    localizer
        .lookup("welcome", Some(&welcome_args()))
        .expect("consumer catalogue should provide the welcome message")
}

/// A `Fluent` bundle holding `message` and left at `Fluent`'s default settings.
fn prebuilt_bundle(message: &str) -> FluentBundle<Arc<FluentResource>> {
    let mut bundle = FluentBundle::new_concurrent(vec![langid!("en-US")]);
    bundle
        .add_resource(Arc::new(
            FluentResource::try_new(message.to_owned()).expect("valid prebuilt resource"),
        ))
        .expect("unique prebuilt resource");
    bundle
}

/// Applies an optional isolation setting to a builder, leaving it untouched for `None`.
fn configured(builder: FluentLocalizerBuilder, setting: Option<bool>) -> FluentLocalizerBuilder {
    match setting {
        Some(value) => builder.with_use_isolating(value),
        None => builder,
    }
}

#[rstest]
#[case::unspecified(None, ISOLATED_WELCOME)]
#[case::enabled(Some(true), ISOLATED_WELCOME)]
#[case::disabled(Some(false), PLAIN_WELCOME)]
fn resource_built_consumer_bundle_renders_the_configured_isolation(
    #[case] setting: Option<bool>,
    #[case] expected: &str,
) {
    let localizer = configured(
        FluentLocalizer::builder(langid!("en-US")).with_consumer_resources([WELCOME_RESOURCE]),
        setting,
    )
    .try_build()
    .expect("valid consumer catalogue");

    assert_eq!(
        render_welcome(&localizer),
        expected,
        "consumer catalogue should render the configured isolation setting"
    );
}

#[rstest]
#[case::unspecified(None, "Usage: \u{2068}acme\u{2069} [OPTIONS] <COMMAND>")]
#[case::enabled(Some(true), "Usage: \u{2068}acme\u{2069} [OPTIONS] <COMMAND>")]
#[case::disabled(Some(false), "Usage: acme [OPTIONS] <COMMAND>")]
fn embedded_defaults_render_the_configured_isolation(
    #[case] setting: Option<bool>,
    #[case] expected: &str,
) {
    let localizer = configured(FluentLocalizer::builder(langid!("en-US")), setting)
        .try_build()
        .expect("embedded English catalogue");
    let mut args = LocalizationArgs::new();
    args.insert("binary", FluentValue::from("acme"));

    assert_eq!(
        localizer.lookup("cli.usage", Some(&args)).as_deref(),
        Some(expected),
        "embedded defaults should render the configured isolation setting"
    );
}

#[rstest]
#[case::disables_isolation_on_a_default_bundle(None, false, PLAIN_WELCOME)]
#[case::restores_isolation_on_an_opted_out_bundle(Some(false), true, ISOLATED_WELCOME)]
fn explicit_setting_outranks_a_prebuilt_bundle(
    #[case] bundle_setting: Option<bool>,
    #[case] requested: bool,
    #[case] expected: &str,
) {
    let mut bundle = prebuilt_bundle(WELCOME_RESOURCE);
    if let Some(value) = bundle_setting {
        bundle.set_use_isolating(value);
    }
    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .disable_defaults()
        .with_consumer_bundle(bundle)
        .with_use_isolating(requested)
        .try_build()
        .expect("prebuilt bundle should build");

    assert_eq!(
        render_welcome(&localizer),
        expected,
        "an explicit setting should outrank the supplied bundle's own isolation"
    );
}

#[test]
fn unspecified_setting_preserves_a_prebuilt_bundle_without_isolation() {
    let mut bundle = prebuilt_bundle(WELCOME_RESOURCE);
    bundle.set_use_isolating(false);
    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .disable_defaults()
        .with_consumer_bundle(bundle)
        .try_build()
        .expect("prebuilt bundle should build");

    assert_eq!(
        render_welcome(&localizer),
        PLAIN_WELCOME,
        "an unspecified setting should leave the supplied bundle untouched"
    );
}

#[test]
fn setter_order_does_not_change_the_rendered_message() {
    let isolation_first = FluentLocalizer::builder(langid!("en-US"))
        .with_use_isolating(false)
        .with_consumer_resources([WELCOME_RESOURCE])
        .try_build()
        .expect("valid consumer catalogue");
    let resources_first = FluentLocalizer::builder(langid!("en-US"))
        .with_consumer_resources([WELCOME_RESOURCE])
        .with_use_isolating(false)
        .try_build()
        .expect("valid consumer catalogue");

    assert_eq!(
        render_welcome(&isolation_first),
        render_welcome(&resources_first),
        "setting isolation before or after the resources should render the same text"
    );
    assert_eq!(
        render_welcome(&resources_first),
        PLAIN_WELCOME,
        "both orders should render the opted-out text"
    );
}

#[test]
fn repeated_calls_keep_the_last_setting() {
    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .with_use_isolating(false)
        .with_use_isolating(true)
        .with_use_isolating(false)
        .with_consumer_resources([WELCOME_RESOURCE])
        .try_build()
        .expect("valid consumer catalogue");

    assert_eq!(
        render_welcome(&localizer),
        PLAIN_WELCOME,
        "the last call to with_use_isolating should win"
    );
}

#[test]
fn disabled_isolation_still_reports_formatting_failures() {
    let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
    let reporter: FormattingIssueReporter = {
        let issues = Arc::clone(&captured);
        Arc::new(move |issue: &FormattingIssue| {
            issues
                .lock()
                .expect("issue log mutex")
                .push(issue.id.clone());
        })
    };

    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .disable_defaults()
        .with_use_isolating(false)
        .with_consumer_resources(["welcome = Hello, { $missing }!"])
        .with_error_reporter(reporter)
        .try_build()
        .expect("valid consumer catalogue");

    assert_eq!(
        localizer.lookup("welcome", None),
        None,
        "a formatting failure should still yield no message"
    );
    assert_eq!(
        *captured.lock().expect("issue log mutex"),
        vec![String::from("welcome")],
        "disabling isolation should not suppress the formatting issue report"
    );
}

#[test]
fn disabled_isolation_still_falls_back_to_embedded_defaults() {
    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .with_use_isolating(false)
        .with_consumer_resources(["welcome = Hello, { $missing }!"])
        .try_build()
        .expect("valid catalogues");
    let mut args = LocalizationArgs::new();
    args.insert("binary", FluentValue::from("acme"));

    assert_eq!(
        localizer.lookup("cli.usage", Some(&args)).as_deref(),
        Some("Usage: acme [OPTIONS] <COMMAND>"),
        "a consumer formatting failure should fall through to the plain default copy"
    );
}
