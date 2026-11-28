//! Steps covering the clap error pipeline end to end.
//!
//! Arrangement installs a real command tree and a localizer; each scenario then
//! parses synthetic argv, records how clap would report the rejection, and
//! asserts on the localized outcome. Kept apart from `localizer_steps` so both
//! modules stay within the repository's 400-line limit.

use super::localizer_steps::{install_fluent_localizer, with_resolved};
use crate::rstest_bdd_support::scenario_state::{ClapOutcome, LocalizerContext};
use anyhow::{Result, anyhow, ensure};
use ortho_config::{LocalizationArgs, LocalizeCmd, Localizer, localize_clap_error};
use rstest_bdd_macros::{given, then, when};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Records lookups so pass-through scenarios can prove none happened.
///
/// Any answer is conspicuous replacement text: if the pipeline consulted the
/// catalogue, the rendered output would visibly change as well as increment the
/// tally.
struct ConspicuousCountingLocalizer {
    lookups: Arc<AtomicUsize>,
}

impl Localizer for ConspicuousCountingLocalizer {
    fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        self.lookups.fetch_add(1, Ordering::SeqCst);
        Some(format!("CONSPICUOUS REPLACEMENT for {id}"))
    }
}

/// Installs [`ConspicuousCountingLocalizer`] and publishes its tally.
fn install_conspicuous_localizer(context: &LocalizerContext) {
    let lookups = Arc::new(AtomicUsize::new(0));
    context.lookup_count.set(Arc::clone(&lookups));
    context
        .localizer
        .set(Box::new(ConspicuousCountingLocalizer { lookups }));
}

/// Restarts the shared lookup tally after arrangement-time catalogue use.
fn reset_lookup_count(context: &LocalizerContext) -> Result<()> {
    context
        .lookup_count
        .with_ref(|count| count.store(0, Ordering::SeqCst))
        .ok_or_else(|| anyhow!("expected a counting localizer"))
}

/// Builds the equals-requiring demo command through the active localizer.
fn localized_equals_command(context: &LocalizerContext) -> Result<clap::Command> {
    context
        .localizer
        .with_ref(|localizer| {
            clap::Command::new("demo")
                .color(clap::ColorChoice::Never)
                .arg(
                    clap::Arg::new("colour")
                        .long("colour")
                        .action(clap::ArgAction::Set)
                        .require_equals(true),
                )
                .localize(localizer.as_ref())
        })
        .ok_or_else(|| anyhow!("localizer must be initialised before localisation"))
}

/// Parses `argv` with the prepared command and localizes the rejection.
fn parse_and_localize(context: &LocalizerContext, argv: &[&str]) -> Result<()> {
    let mut command = context
        .command
        .take()
        .ok_or_else(|| anyhow!("expected a prepared command"))?;
    let error = command
        .try_get_matches_from_mut(argv)
        .expect_err("the synthetic argv should be rejected by clap");
    let kind = format!("{:?}", error.kind());
    let stock = ClapOutcome::from_error(&error);
    let localized = context
        .localizer
        .with_ref(|localizer| localize_clap_error(error, localizer.as_ref()))
        .ok_or_else(|| anyhow!("localizer must be initialised before localisation"))?;

    context.error_kind.set(kind);
    context.stock_outcome.set(stock);
    context
        .localized_outcome
        .set(ClapOutcome::from_error(&localized));
    context.resolved.set(localized.to_string());
    Ok(())
}

#[given("a localized command requiring equals for its option")]
fn localized_command_requiring_equals(context: &LocalizerContext) -> Result<()> {
    install_fluent_localizer(context, &[], false)?;
    context.command.set(localized_equals_command(context)?);
    Ok(())
}

#[given("a localized command showing help when arguments are absent")]
fn localized_command_showing_help(context: &LocalizerContext) -> Result<()> {
    install_conspicuous_localizer(context);
    let command = context
        .localizer
        .with_ref(|localizer| {
            clap::Command::new("demo")
                .color(clap::ColorChoice::Never)
                .arg_required_else_help(true)
                .arg(clap::Arg::new("path").required(true))
                .localize(localizer.as_ref())
        })
        .ok_or_else(|| anyhow!("localizer must be initialised before localisation"))?;
    // Preparing the localized command tree is itself a catalogue consumer, so
    // restart the tally: the scenario asserts about the error pipeline alone.
    reset_lookup_count(context)?;
    context.command.set(command);
    Ok(())
}

#[given("an incomplete locale and the embedded en-US catalogue")]
fn incomplete_locale_with_embedded_catalogue(context: &LocalizerContext) -> Result<()> {
    install_fluent_localizer(context, &["cli.about = Consumer about only"], true)
}

#[when("I parse that option without equals")]
fn parse_option_without_equals(context: &LocalizerContext) -> Result<()> {
    parse_and_localize(context, &["demo", "--colour", "red"])
}

#[when("I parse no arguments")]
fn parse_no_arguments(context: &LocalizerContext) -> Result<()> {
    parse_and_localize(context, &["demo"])
}

#[when("I trigger a newly covered parser error")]
fn trigger_newly_covered_parser_error(context: &LocalizerContext) -> Result<()> {
    context.command.set(localized_equals_command(context)?);
    parse_and_localize(context, &["demo", "--colour", "red"])
}

#[then("the error kind is {kind}")]
fn error_kind_is(context: &LocalizerContext, kind: String) -> Result<()> {
    let actual = context
        .error_kind
        .take()
        .ok_or_else(|| anyhow!("expected a recorded clap error kind"))?;
    ensure!(
        actual == kind,
        "expected error kind {kind}, recorded {actual}"
    );
    Ok(())
}

#[then("the en-US no-equals translation is rendered")]
fn no_equals_translation_is_rendered(context: &LocalizerContext) -> Result<()> {
    with_resolved(context, false, |actual| {
        ensure!(
            actual.contains("Equal sign is required"),
            "expected the en-US no-equals translation, got {actual:?}"
        );
        ensure!(
            actual.contains("--colour"),
            "expected the argument label in the translation, got {actual:?}"
        );
        Ok(())
    })
}

#[then("the localizer is not called")]
fn localizer_is_not_called(context: &LocalizerContext) -> Result<()> {
    let lookups = context
        .lookup_count
        .with_ref(|count| count.load(Ordering::SeqCst))
        .ok_or_else(|| anyhow!("expected a counting localizer"))?;
    ensure!(
        lookups == 0,
        "display-only errors must bypass the catalogue, but it was consulted {lookups} times"
    );
    Ok(())
}

#[then("the output and exit status equal stock clap")]
fn output_and_status_equal_stock(context: &LocalizerContext) -> Result<()> {
    let stock = context
        .stock_outcome
        .take()
        .ok_or_else(|| anyhow!("expected the stock clap outcome"))?;
    let localized = context
        .localized_outcome
        .take()
        .ok_or_else(|| anyhow!("expected the localized clap outcome"))?;
    ensure!(
        localized.text == stock.text,
        "localized text differs from stock clap:\n{}\n---\n{}",
        localized.text,
        stock.text
    );
    ensure!(
        localized.exit_code == stock.exit_code,
        "localized exit code {} differs from stock clap {}",
        localized.exit_code,
        stock.exit_code
    );
    ensure!(
        localized.uses_stderr == stock.uses_stderr,
        "localized stream selection differs from stock clap"
    );
    Ok(())
}

#[then("the embedded en-US translation is rendered without formatting errors")]
fn embedded_translation_has_no_formatting_errors(context: &LocalizerContext) -> Result<()> {
    let issues = context.take_issues();
    ensure!(
        issues.is_empty(),
        "unexpected formatting issues: {issues:?}"
    );
    with_resolved(context, false, |actual| {
        ensure!(
            actual.contains("Equal sign is required"),
            "expected the embedded en-US translation, got {actual:?}"
        );
        ensure!(
            !actual.contains('{'),
            "no placeholder should remain unresolved: {actual:?}"
        );
        Ok(())
    })
}
