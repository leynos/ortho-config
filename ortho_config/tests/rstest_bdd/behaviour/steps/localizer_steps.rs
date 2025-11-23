//! Steps covering the localisation helper surfaces.

use crate::fixtures::LocalizerContext;
use anyhow::{Result, anyhow, ensure};
use fluent_bundle::FluentValue;
use ortho_config::{LocalizationArgs, Localizer, NoOpLocalizer};
use rstest_bdd_macros::{given, then, when};
use std::collections::HashMap;

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

#[given("a noop localiser")]
fn noop_localizer(context: &LocalizerContext) {
    context.localizer.set(Box::new(NoOpLocalizer::new()));
}

#[given("a subject-aware localiser")]
fn subject_localizer(context: &LocalizerContext) {
    context.localizer.set(Box::new(SubjectLocalizer));
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

#[then("the localised text is {expected}")]
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
