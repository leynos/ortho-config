//! Tests covering the CLI localisation helpers.

use super::super::{CommandLine, Commands, LocalizeCmd};
use crate::localizer::DemoLocalizer;
use anyhow::{Context, Result, anyhow, ensure};
use clap::CommandFactory;
use ortho_config::{FluentLocalizerError, LanguageIdentifier, langid, parse_localized_command};
use rstest::{fixture, rstest};

#[fixture]
fn demo_localizer(
    #[default(langid!("en-US"))] locale: LanguageIdentifier,
) -> Result<DemoLocalizer, FluentLocalizerError> {
    DemoLocalizer::try_for_locale(locale)
}

fn assert_expected_copy(demo_localizer: &DemoLocalizer) -> Result<()> {
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(demo_localizer);
    let about_text = command
        .get_about()
        .ok_or_else(|| anyhow!("about text should be set"))?
        .to_string();
    ensure!(
        about_text.contains("localised"),
        "expected demo copy in about: {about_text}"
    );

    let long_about_text = command
        .get_long_about()
        .ok_or_else(|| anyhow!("long about text should be set"))?
        .to_string();
    ensure!(
        long_about_text.contains(command.get_name()),
        "expected the command name in the long about: {long_about_text}"
    );
    Ok(())
}

fn assert_localized_tree(demo_localizer: &DemoLocalizer) -> Result<()> {
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(demo_localizer);
    let greet = command
        .get_subcommands()
        .find(|sub| sub.get_name() == "greet")
        .ok_or_else(|| anyhow!("greet subcommand must exist"))?;
    let about_text = greet
        .get_about()
        .ok_or_else(|| anyhow!("greet about should be localized"))?
        .to_string();
    ensure!(
        about_text.contains("friendly greeting"),
        "expected demo copy in the greet about: {about_text}"
    );
    Ok(())
}

fn assert_cli_builds(demo_localizer: &DemoLocalizer) -> Result<()> {
    let args = ["hello-world", "greet"];
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(demo_localizer);
    let (cli, _matches) =
        parse_localized_command::<CommandLine, _, _>(command, args, demo_localizer)
            .context("CLI should parse after localization")?;
    ensure!(
        matches!(cli.command, Commands::Greet(_)),
        "expected the greet subcommand to be selected"
    );
    Ok(())
}

fn assert_localized_error(demo_localizer: &DemoLocalizer) -> Result<()> {
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(demo_localizer);
    let err =
        parse_localized_command::<CommandLine, _, _>(command, ["hello-world"], demo_localizer)
            .err()
            .ok_or_else(|| anyhow!("missing subcommand should be reported"))?;
    let rendered = err.to_string();
    ensure!(
        rendered.contains("Pick a workflow"),
        "expected consumer translation in error output: {rendered}"
    );
    ensure!(
        rendered.contains("greet") && rendered.contains("take-leave"),
        "expected available subcommands to flow into translation: {rendered}"
    );
    Ok(())
}

#[rstest]
fn command_with_localizer_overrides_copy(
    #[from(demo_localizer)] localizer_result: Result<DemoLocalizer, FluentLocalizerError>,
) -> Result<()> {
    let demo_localizer = localizer_result?;
    assert_expected_copy(&demo_localizer)
}

#[rstest]
fn localizes_subcommand_tree(
    #[from(demo_localizer)] localizer_result: Result<DemoLocalizer, FluentLocalizerError>,
) -> Result<()> {
    let demo_localizer = localizer_result?;
    assert_localized_tree(&demo_localizer)
}

#[rstest]
fn try_parse_with_localizer_builds_cli(
    #[from(demo_localizer)] localizer_result: Result<DemoLocalizer, FluentLocalizerError>,
) -> Result<()> {
    let demo_localizer = localizer_result?;
    assert_cli_builds(&demo_localizer)
}

#[rstest]
fn try_parse_with_localizer_localises_errors(
    #[from(demo_localizer)] localizer_result: Result<DemoLocalizer, FluentLocalizerError>,
) -> Result<()> {
    let demo_localizer = localizer_result?;
    assert_localized_error(&demo_localizer)
}

#[rstest]
fn demo_localizer_propagates_unsupported_locale(
    #[with(langid!("fr"))] demo_localizer: Result<DemoLocalizer, FluentLocalizerError>,
) {
    assert!(matches!(
        demo_localizer,
        Err(FluentLocalizerError::UnsupportedLocale { locale }) if locale == langid!("fr")
    ));
}

#[test]
fn noop_localizer_keeps_stock_error_messages() {
    let baseline = CommandLine::command()
        .try_get_matches_from(["hello-world"])
        .expect_err("default clap parsing should fail without subcommand")
        .to_string();

    let noop_localizer = DemoLocalizer::noop();
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(&noop_localizer);
    let err =
        parse_localized_command::<CommandLine, _, _>(command, ["hello-world"], &noop_localizer)
            .expect_err("expected parse failure to bubble up");

    assert_eq!(
        err.to_string(),
        baseline,
        "noop localizer should preserve clap error formatting"
    );
}

#[test]
fn command_with_noop_localizer_uses_stock_clap_strings() {
    let mut default_command = CommandLine::command();

    let noop_localizer = DemoLocalizer::noop();
    let mut noop_localized_command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(&noop_localizer);

    let default_about = default_command
        .get_about()
        .expect("about text should be set for default command")
        .to_string();
    let noop_about = noop_localized_command
        .get_about()
        .expect("about text should be set for noop-localised command")
        .to_string();
    assert_eq!(
        default_about, noop_about,
        "DemoLocalizer::noop() should not change the about text"
    );

    let default_usage = default_command.render_usage();
    let noop_usage = noop_localized_command.render_usage();
    assert_eq!(
        default_usage, noop_usage,
        "DemoLocalizer::noop() should not change the usage text"
    );
}
