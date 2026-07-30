//! Tests covering the CLI localisation helpers.

use super::super::{CommandLine, Commands, LocalizeCmd};
use crate::localizer::DemoLocalizer;
use clap::CommandFactory;
use ortho_config::{FluentLocalizerError, parse_localized_command};
use rstest::{fixture, rstest};

#[fixture]
fn demo_localizer() -> Result<DemoLocalizer, FluentLocalizerError> {
    DemoLocalizer::try_new()
}

#[track_caller]
fn assert_expected_copy(demo_localizer: &DemoLocalizer) {
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(demo_localizer);
    let optional_about = command.get_about();
    assert!(optional_about.is_some(), "about text should be set");
    let about_text = optional_about.map(ToString::to_string).unwrap_or_default();
    assert!(
        about_text.contains("localised"),
        "expected demo copy in about"
    );

    let optional_long_about = command.get_long_about();
    assert!(
        optional_long_about.is_some(),
        "long about text should be set"
    );
    let long_about_text = optional_long_about
        .map(ToString::to_string)
        .unwrap_or_default();
    assert!(long_about_text.contains(command.get_name()));
}

#[track_caller]
fn assert_localized_tree(demo_localizer: &DemoLocalizer) {
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(demo_localizer);
    let greet = command
        .get_subcommands()
        .find(|sub| sub.get_name() == "greet");
    assert!(greet.is_some(), "greet subcommand must exist");
    let optional_about = greet.and_then(clap::Command::get_about);
    assert!(optional_about.is_some(), "greet about should be localized");
    let about_text = optional_about.map(ToString::to_string).unwrap_or_default();
    assert!(about_text.contains("friendly greeting"));
}

#[track_caller]
fn assert_cli_builds(demo_localizer: &DemoLocalizer) {
    let args = ["hello-world", "greet"];
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(demo_localizer);
    let parsed = parse_localized_command::<CommandLine, _, _>(command, args, demo_localizer);
    assert!(parsed.is_ok(), "expected to parse args");
    let Ok((cli, _matches)) = parsed else {
        return;
    };
    assert!(matches!(cli.command, Commands::Greet(_)));
}

#[track_caller]
fn assert_localized_error(demo_localizer: &DemoLocalizer) {
    let command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(demo_localizer);
    let parsed =
        parse_localized_command::<CommandLine, _, _>(command, ["hello-world"], demo_localizer);
    assert!(parsed.is_err(), "missing subcommand should be reported");
    let Err(err) = parsed else {
        return;
    };
    let rendered = err.to_string();
    assert!(
        rendered.contains("Pick a workflow"),
        "expected consumer translation in error output: {rendered}"
    );
    assert!(
        rendered.contains("greet") && rendered.contains("take-leave"),
        "expected available subcommands to flow into translation: {rendered}"
    );
}

#[rstest]
fn command_with_localizer_overrides_copy(
    #[from(demo_localizer)] localizer_result: Result<DemoLocalizer, FluentLocalizerError>,
) -> Result<(), FluentLocalizerError> {
    let demo_localizer = localizer_result?;
    assert_expected_copy(&demo_localizer);
    Ok(())
}

#[rstest]
fn localizes_subcommand_tree(
    #[from(demo_localizer)] localizer_result: Result<DemoLocalizer, FluentLocalizerError>,
) -> Result<(), FluentLocalizerError> {
    let demo_localizer = localizer_result?;
    assert_localized_tree(&demo_localizer);
    Ok(())
}

#[rstest]
fn try_parse_with_localizer_builds_cli(
    #[from(demo_localizer)] localizer_result: Result<DemoLocalizer, FluentLocalizerError>,
) -> Result<(), FluentLocalizerError> {
    let demo_localizer = localizer_result?;
    assert_cli_builds(&demo_localizer);
    Ok(())
}

#[rstest]
fn try_parse_with_localizer_localises_errors(
    #[from(demo_localizer)] localizer_result: Result<DemoLocalizer, FluentLocalizerError>,
) -> Result<(), FluentLocalizerError> {
    let demo_localizer = localizer_result?;
    assert_localized_error(&demo_localizer);
    Ok(())
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
