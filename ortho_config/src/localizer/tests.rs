use super::*;
use crate::langid;
use rstest::rstest;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[rstest]
fn noop_localizer_relies_on_fallback() {
    let localizer = NoOpLocalizer::new();
    let resolved = localizer.message("cli.about", None, "fallback");
    assert_eq!(resolved, "fallback");
}

struct StubLocalizer;

impl Localizer for StubLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        Some(args.map_or_else(
            || format!("{id}:no-args"),
            |values| {
                let subject = values
                    .get("subject")
                    .and_then(|value| match value {
                        FluentValue::String(text) => Some(text.to_string()),
                        _ => None,
                    })
                    .unwrap_or_else(|| String::from("<missing>"));
                format!("{id}:{subject}")
            },
        ))
    }
}

#[rstest]
fn stub_localizer_uses_args() {
    let localizer = StubLocalizer;
    let mut args: LocalizationArgs<'static> = HashMap::new();
    args.insert("subject", FluentValue::from("hello"));
    let resolved = localizer.message("cli.about", Some(&args), "fallback");
    assert_eq!(resolved, "cli.about:hello");
}

#[rstest]
fn fluent_localizer_prefers_consumer_catalogue() {
    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .with_consumer_resources(["cli-about = Consumer about text"])
        .try_build()
        .expect("consumer bundle should build");

    let resolved = localizer.lookup("cli.about", None);
    assert_eq!(resolved.as_deref(), Some("Consumer about text"));
}

#[rstest]
fn fluent_localizer_falls_back_to_default_when_consumer_missing() {
    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .with_consumer_resources(["unrelated = no-op"])
        .try_build()
        .expect("default bundle should build");

    let resolved = localizer.lookup("cli.about", None);
    assert!(
        resolved
            .as_ref()
            .is_some_and(|text| text.contains("OrthoConfig"))
    );
}

#[rstest]
fn fluent_localizer_logs_and_falls_back_on_format_error() {
    let issues = Arc::new(Mutex::new(Vec::new()));
    let reporter: FormattingIssueReporter = {
        let captured_issues = Arc::clone(&issues);
        Arc::new(move |issue: &FormattingIssue| {
            let mut guard = captured_issues.lock().expect("issue log mutex poisoned");
            guard.push(issue.id.clone());
        })
    };

    let mut args: LocalizationArgs<'static> = HashMap::new();
    args.insert("binary", FluentValue::from("demo"));

    let localizer = FluentLocalizer::builder(langid!("en-US"))
        .with_consumer_resources(["cli-usage = Usage: { $binary } and { $missing }"])
        .with_error_reporter(reporter)
        .try_build()
        .expect("consumer bundle should build");

    let resolved = localizer.message("cli.usage", Some(&args), "fallback usage");
    let sanitised = strip_bidi_isolates(&resolved);
    assert!(sanitised.starts_with("Usage: demo"), "resolved: {resolved}");

    let logged = issues
        .lock()
        .expect("issue log mutex poisoned during assertion");
    assert_eq!(*logged, vec![String::from("cli.usage")]);
}

fn strip_bidi_isolates(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}
