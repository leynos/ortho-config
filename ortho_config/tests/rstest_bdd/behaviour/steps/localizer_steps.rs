//! Steps covering the localisation helper surfaces.

use super::value_parsing::{normalize_scalar, strip_isolates};
use crate::scenario_state::{LocalizedDemoArgs, LocalizerContext};
use anyhow::{Result, anyhow, ensure};
use clap::CommandFactory;
use fluent_bundle::FluentValue;
use ortho_config::{
    FluentLocalizer, FormattingIssue, LocalizationArgs, LocalizeCmd, Localizer, NoOpLocalizer,
    OrthoConfigLocalization, langid, localize_clap_error, localize_clap_error_with_command,
    parse_localized_command,
};
use rstest_bdd_macros::{given, then, when};
use std::collections::HashMap;
use std::sync::Arc;

mod support_localizers {
    //! Localizer doubles shared by the behaviour steps.

    use ortho_config as crate_root;
    include!("../../../support/localizers.rs");
}
use support_localizers::ArgumentEchoLocalizer;

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

#[given("a clap-aware localizer")]
fn clap_aware_localizer(context: &LocalizerContext) {
    context
        .localizer
        .set(Box::new(ArgumentEchoLocalizer::new("<missing>")));
}

#[given("a fluent localizer with consumer overrides")]
fn fluent_localizer(context: &LocalizerContext) -> Result<()> {
    install_fluent_localizer(
        context,
        &["cli.about = Localised about from consumer"],
        false,
    )
}

#[given("a fluent localizer with a mismatched template")]
fn fluent_localizer_with_error(context: &LocalizerContext) -> Result<()> {
    install_fluent_localizer(
        context,
        &["cli.usage = Usage: { $binary } and { $missing }"],
        true,
    )
}

#[when("I request id {id} with fallback {fallback}")]
fn request_without_args(context: &LocalizerContext, id: String, fallback: String) -> Result<()> {
    let resolved = context
        .localizer
        .with_ref(|localizer| localizer.message(&id, None, &fallback))
        .ok_or_else(|| anyhow!("localizer must be initialised"))?;
    context.resolved.set(resolved);
    Ok(())
}

/// Requests a localised message using a single Fluent argument key/value pair.
fn request_with_arg<T>(
    context: &LocalizerContext,
    id: String,
    arg_name: &str,
    arg_value: T,
) -> Result<()>
where
    T: AsRef<str>,
{
    let resolved = context
        .localizer
        .with_ref(|localizer| {
            let mut args: LocalizationArgs<'_> = HashMap::new();
            args.insert(arg_name, FluentValue::from(arg_value.as_ref()));
            let fallback_text = format!("missing:{id}");
            localizer.message(&id, Some(&args), &fallback_text)
        })
        .ok_or_else(|| anyhow!("localizer must be initialised"))?;
    context.resolved.set(resolved);
    Ok(())
}

#[when("I request id {id} for subject {subject}")]
fn request_with_subject(context: &LocalizerContext, id: String, subject: String) -> Result<()> {
    request_with_arg(context, id, "subject", subject)
}

#[when("I request id {id} for binary {binary}")]
fn request_with_binary(context: &LocalizerContext, id: String, binary: String) -> Result<()> {
    request_with_arg(context, id, "binary", binary)
}

#[when("I parse a demo command without a subcommand")]
fn parse_demo_command_without_subcommand(context: &LocalizerContext) -> Result<()> {
    let mut command = clap::Command::new("demo")
        .subcommand_required(true)
        .subcommand(clap::Command::new("greet"))
        .subcommand(clap::Command::new("take-leave"));

    let error = command
        .try_get_matches_from_mut(["demo"])
        .expect_err("missing subcommand should produce a clap error");

    context.baseline_error.set(error.to_string());
    let rendered = context
        .localizer
        .with_ref(|localizer| {
            localize_clap_error_with_command(error, localizer.as_ref(), Some(&command)).to_string()
        })
        .ok_or_else(|| anyhow!("localizer must be initialised before localisation"))?;
    context.resolved.set(rendered);
    Ok(())
}

#[when("I parse a demo command missing an argument")]
fn parse_demo_command_missing_argument(context: &LocalizerContext) -> Result<()> {
    let mut command = clap::Command::new("demo").arg(clap::Arg::new("path").required(true));
    let error = command
        .try_get_matches_from_mut(["demo"])
        .expect_err("missing argument should produce a clap error");
    context.baseline_error.set(error.to_string());
    let rendered = context
        .localizer
        .with_ref(|localizer| {
            localize_clap_error_with_command(error, localizer.as_ref(), Some(&command)).to_string()
        })
        .ok_or_else(|| anyhow!("localizer must be initialised before localisation"))?;
    context.resolved.set(rendered);
    Ok(())
}

#[given("a clap error for a missing argument")]
fn clap_error_for_missing_argument(context: &LocalizerContext) {
    let mut command =
        clap::Command::new("bdd").arg(clap::Arg::new("config").required(true).value_name("CONFIG"));

    let error = command
        .try_get_matches_from_mut(["bdd"])
        .expect_err("expected clap error for missing argument");

    let argument = extract_argument(&error);
    context.argument_label.set(argument);
    context.baseline_error.set(error.to_string());
    context.clap_error.set(error);
}

#[when("I localize the clap error")]
fn localize_clap_error_step(context: &LocalizerContext) -> Result<()> {
    let error = context
        .clap_error
        .take()
        .ok_or_else(|| anyhow!("expected clap error to be initialised"))?;
    let rendered = context
        .localizer
        .with_ref(|localizer| localize_clap_error(error, localizer.as_ref()).to_string())
        .ok_or_else(|| anyhow!("localizer must be initialised before localisation"))?;
    context.resolved.set(rendered);
    Ok(())
}

/// Executes an assertion against the last resolved message, optionally restoring it.
fn with_resolved<R, F>(context: &LocalizerContext, preserve: bool, f: F) -> Result<R>
where
    F: FnOnce(&str) -> Result<R>,
{
    let actual = context
        .resolved
        .take()
        .ok_or_else(|| anyhow!("expected a resolved message"))?;
    let result = f(&actual)?;
    if preserve {
        context.resolved.set(actual);
    }
    Ok(result)
}

#[then("the localized text is {expected}")]
fn assert_localised(context: &LocalizerContext, expected: String) -> Result<()> {
    let expected = normalize_scalar(&expected);
    with_resolved(context, false, |actual| {
        let actual = strip_isolates(actual);
        ensure!(
            actual == expected,
            "resolved {actual:?}; expected {expected:?}",
        );
        Ok(())
    })
}

#[then("a localisation formatting error is recorded")]
fn assert_formatting_issue_logged(context: &LocalizerContext) -> Result<()> {
    let issues = context.take_issues();
    ensure!(
        issues == ["cli.usage"],
        "expected only the cli.usage formatting issue, captured {issues:?}"
    );
    Ok(())
}

#[then("the localized text contains {expected}")]
fn localized_text_contains(context: &LocalizerContext, expected: String) -> Result<()> {
    with_resolved(context, true, |actual| {
        ensure!(
            actual.contains(&expected),
            "expected '{expected:?}' to appear in {actual:?}"
        );
        Ok(())
    })
}

#[then("the localized text matches the baseline clap output")]
fn localized_text_matches_baseline(context: &LocalizerContext) -> Result<()> {
    let baseline = context
        .baseline_error
        .take()
        .ok_or_else(|| anyhow!("expected baseline clap output to be recorded"))?;
    with_resolved(context, false, |actual| {
        ensure!(
            actual == baseline,
            "expected localisation fallback to keep clap message"
        );
        Ok(())
    })
}

#[then("the localized text includes the clap argument label")]
fn localized_text_includes_argument(context: &LocalizerContext) -> Result<()> {
    let argument = context
        .argument_label
        .take()
        .ok_or_else(|| anyhow!("expected argument label to be captured"))?;
    with_resolved(context, true, |actual| {
        ensure!(
            actual.contains(&argument),
            "expected argument label {argument:?} in {actual:?}"
        );
        Ok(())
    })
}

/// Builds a Fluent localizer over `resources` and installs it into `context`.
///
/// Arrangement is fallible, so the builder error propagates to the calling step
/// rather than panicking here: this helper is not a test, and a failure to build
/// the localizer is a setup fault the step should report.
fn install_fluent_localizer(
    context: &LocalizerContext,
    resources: &[&'static str],
    capture_errors: bool,
) -> Result<()> {
    let mut builder = FluentLocalizer::builder(langid!("en-US"))
        .with_consumer_resources(resources.iter().copied());

    if capture_errors {
        let issues = context
            .issues
            .with_ref(Arc::clone)
            .unwrap_or_else(|| Arc::new(std::sync::Mutex::new(Vec::new())));
        builder = builder.with_error_reporter(Arc::new(move |issue: &FormattingIssue| {
            if let Ok(mut guard) = issues.lock() {
                guard.push(issue.id.clone());
            }
        }));
    }

    let localizer = builder.try_build()?;
    context.localizer.set(Box::new(localizer));
    Ok(())
}

fn extract_argument(error: &clap::Error) -> String {
    match error.get(clap::error::ContextKind::InvalidArg) {
        Some(clap::error::ContextValue::Strings(values)) => values.join(", "),
        Some(clap::error::ContextValue::String(value)) => value.clone(),
        Some(other) => other.to_string(),
        None => "<missing>".to_owned(),
    }
}

#[given("a derived configuration struct with a localization catalogue")]
fn derived_config_localization_catalogue(context: &LocalizerContext) -> Result<()> {
    let recipient = LocalizedDemoArgs::ARG_IDS
        .iter()
        .find(|arg| arg.name == "recipient")
        .ok_or_else(|| anyhow!("recipient argument id missing from derived constants"))?;
    let resource = format!(
        "{} = Localised demo about\n{} = Localised recipient help\n",
        LocalizedDemoArgs::ABOUT_ID,
        recipient.help_id
    );
    let resource: &'static str = Box::leak(resource.into_boxed_str());
    install_fluent_localizer(context, &[resource], false)
}

#[when("the command line is parsed with a Fluent localizer")]
fn parse_localized_demo_command(context: &LocalizerContext) -> Result<()> {
    let (about, parsed) = context
        .localizer
        .with_ref(|localizer| {
            let command = LocalizedDemoArgs::command()
                .with_base(LocalizedDemoArgs::LOCALIZATION_BASE)
                .localize(localizer.as_ref());
            let about = command
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_default();
            let parsed = parse_localized_command::<LocalizedDemoArgs, _, _>(
                command,
                ["localized-demo", "--recipient", "Ada"],
                localizer.as_ref(),
            );
            (about, parsed)
        })
        .ok_or_else(|| anyhow!("localizer must be initialised before localisation"))?;
    let _parsed = parsed.map_err(|error| anyhow!("localized parse failed: {error}"))?;
    context.resolved.set(about);
    Ok(())
}

#[then("the help text resolves through the derive-generated identifiers")]
fn help_text_resolves_through_derived_ids(context: &LocalizerContext) -> Result<()> {
    with_resolved(context, false, |actual| {
        ensure!(
            actual == "Localised demo about",
            "expected help resolved through the derived identifiers, got {actual:?}"
        );
        Ok(())
    })
}
