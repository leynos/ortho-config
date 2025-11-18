//! Steps covering the localisation helper surfaces.

use crate::fixtures::LocalizerContext;
use anyhow::{Result, anyhow, ensure};
use fluent_bundle::FluentValue;
use ortho_config::{Localizer, LocalizationArgs, NoOpLocalizer};
use rstest_bdd_macros::{given, then, when};
use std::collections::HashMap;

#[derive(Debug)]
struct SubjectLocalizer;

impl Localizer for SubjectLocalizer {
    fn get_message(&self, id: &str) -> Option<String> {
        Some(format!("{id}:no-args"))
    }

    fn get_message_with_args(
        &self,
        id: &str,
        args: Option<&LocalizationArgs<'_>>,
    ) -> Option<String> {
        let values = args?;
        let subject = values
            .get("subject")
            .and_then(|value| match value {
                FluentValue::String(text) => Some(text.to_string()),
                _ => None,
            })?;
        Some(format!("Hola, {subject}! ({id})"))
    }
}

#[given("a noop localiser")]
fn noop_localizer(context: &LocalizerContext) {
    context
        .localizer
        .set(Box::new(NoOpLocalizer::new()));
}

#[given("a subject-aware localiser")]
fn subject_localizer(context: &LocalizerContext) {
    context.localizer.set(Box::new(SubjectLocalizer));
}

#[when("I request id {id} with fallback {fallback}")]
fn request_without_args(context: &LocalizerContext, id: String, fallback: String) -> Result<()> {
    let resolved = context
        .localizer
        .with_ref(|localizer| localizer.message_or(&id, fallback.as_str()))
        .ok_or_else(|| anyhow!("localizer must be initialised"))?;
    context.resolved.set(resolved);
    Ok(())
}

#[when("I request id {id} for subject {subject}")]
fn request_with_subject(
    context: &LocalizerContext,
    id: String,
    subject: String,
) -> Result<()> {
    let resolved = context
        .localizer
        .with_ref(|localizer| {
            let mut args: LocalizationArgs<'_> = HashMap::new();
            args.insert("subject", FluentValue::from(subject.as_str()));
            let fallback_text = format!("missing:{id}");
            localizer.message_with_args_or(&id, Some(&args), fallback_text.as_str())
        })
        .ok_or_else(|| anyhow!("localizer must be initialised"))?;
    context.resolved.set(resolved);
    Ok(())
}

#[then("the localised text is {expected}")]
fn assert_localised(context: &LocalizerContext, expected: String) -> Result<()> {
    let actual = context
        .resolved
        .take()
        .ok_or_else(|| anyhow!("expected a resolved message"))?;
    ensure!(actual == expected, "resolved {actual:?}; expected {expected:?}");
    Ok(())
}
