//! Steps covering the localisation helper surfaces.

use crate::fixtures::LocalizerContext;
use anyhow::{Result, anyhow, ensure};
use fluent_bundle::FluentValue;
use ortho_config::{
    langid, FluentLocalizer, FormattingIssue, LocalizationArgs, Localizer, NoOpLocalizer,
};
use rstest_bdd_macros::{given, then, when};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct MessageId(String);

impl From<String> for MessageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for MessageId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
struct FallbackText(String);

impl From<String> for FallbackText {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for FallbackText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
struct SubjectName(String);

impl From<String> for SubjectName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for SubjectName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
struct ExpectedText(String);

impl From<String> for ExpectedText {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for ExpectedText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
struct BinaryName(String);

impl From<String> for BinaryName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for BinaryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
struct SubjectLocalizer;

impl Localizer for SubjectLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        if let Some(values) = args {
            let subject = values.get("subject").and_then(|value| match value {
                FluentValue::String(text) => Some(text.to_string()),
                _ => None,
            })?;
            Some(format!("Hola, {subject}! ({id})"))
        } else {
            Some(format!("{id}:no-args"))
        }
    }
}

#[given("a noop localizer")]
fn noop_localizer(context: &LocalizerContext) {
    context.localizer.set(Box::new(NoOpLocalizer::new()));
}

#[given("a subject-aware localizer")]
fn subject_localizer(context: &LocalizerContext) {
    context.localizer.set(Box::new(SubjectLocalizer));
}

#[given("a fluent localizer with consumer overrides")]
fn fluent_localizer(context: &LocalizerContext) {
    install_fluent_localizer(
        context,
        &["cli.about = Localised about from consumer"],
        false,
    );
}

#[given("a fluent localizer with a mismatched template")]
fn fluent_localizer_with_error(context: &LocalizerContext) {
    install_fluent_localizer(
        context,
        &["cli.usage = Usage: { $binary } and { $missing }"],
        true,
    );
}

#[when("I request id {id} with fallback {fallback}")]
fn request_without_args(
    context: &LocalizerContext,
    id: MessageId,
    fallback: FallbackText,
) -> Result<()> {
    let resolved = context
        .localizer
        .with_ref(|localizer| localizer.message(id.as_ref(), None, fallback.as_ref()))
        .ok_or_else(|| anyhow!("localizer must be initialised"))?;
    context.resolved.set(resolved);
    Ok(())
}

#[when("I request id {id} for subject {subject}")]
fn request_with_subject(
    context: &LocalizerContext,
    id: MessageId,
    subject: SubjectName,
) -> Result<()> {
    let resolved = context
        .localizer
        .with_ref(|localizer| {
            let mut args: LocalizationArgs<'_> = HashMap::new();
            args.insert("subject", FluentValue::from(subject.as_ref()));
            let fallback_text = format!("missing:{}", id.as_ref());
            localizer.message(id.as_ref(), Some(&args), fallback_text.as_str())
        })
        .ok_or_else(|| anyhow!("localizer must be initialised"))?;
    context.resolved.set(resolved);
    Ok(())
}

#[when("I request id {id} for binary {binary}")]
fn request_with_binary(
    context: &LocalizerContext,
    id: MessageId,
    binary: BinaryName,
) -> Result<()> {
    let resolved = context
        .localizer
        .with_ref(|localizer| {
            let mut args: LocalizationArgs<'_> = HashMap::new();
            args.insert("binary", FluentValue::from(binary.as_ref()));
            let fallback_text = format!("missing:{}", id.as_ref());
            localizer.message(id.as_ref(), Some(&args), fallback_text.as_str())
        })
        .ok_or_else(|| anyhow!("localizer must be initialised"))?;
    context.resolved.set(resolved);
    Ok(())
}

#[then("the localized text is {expected}")]
fn assert_localised(context: &LocalizerContext, expected: ExpectedText) -> Result<()> {
    let actual = context
        .resolved
        .take()
        .ok_or_else(|| anyhow!("expected a resolved message"))?;
    ensure!(
        actual == expected.as_ref(),
        "resolved {actual:?}; expected {:?}",
        expected.as_ref()
    );
    Ok(())
}

#[then("a localisation formatting error is recorded")]
fn assert_formatting_issue_logged(context: &LocalizerContext) -> Result<()> {
    let issues = context.take_issues();
    ensure!(
        !issues.is_empty(),
        "expected at least one formatting issue to be captured"
    );
    Ok(())
}

fn install_fluent_localizer(
    context: &LocalizerContext,
    resources: &[&'static str],
    capture_errors: bool,
) {
    let mut builder = FluentLocalizer::builder(langid!("en-US"))
        .with_consumer_resources(resources.iter().copied());

    if capture_errors {
        let ctx = context.clone();
        builder = builder.with_error_reporter(Arc::new(move |issue: &FormattingIssue| {
            ctx.record_issue(issue.id.clone());
        }));
    }

    let localizer = builder
        .try_build()
        .expect("fluent localizer should be constructed in tests");
    context.localizer.set(Box::new(localizer));
}
