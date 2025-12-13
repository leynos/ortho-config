//! Tests covering the CLI localisation helpers.

use super::super::{CommandLine, Commands, LocalizeCmd};
use crate::localizer::DemoLocalizer;
use clap::CommandFactory;
use rstest::{fixture, rstest};

#[fixture]
fn demo_localizer() -> DemoLocalizer {
    DemoLocalizer::try_new().expect("demo localiser should build")
}

#[rstest]
fn command_with_localizer_overrides_copy(demo_localizer: DemoLocalizer) {
    let command = CommandLine::command().localize(&demo_localizer);
    let about = command
        .get_about()
        .expect("about text should be set")
        .to_string();
    assert!(about.contains("localised"), "expected demo copy in about");

    let long_about = command
        .get_long_about()
        .expect("long about text should be set")
        .to_string();
    assert!(long_about.contains(command.get_name()));
}

#[rstest]
fn localizes_subcommand_tree(demo_localizer: DemoLocalizer) {
    let command = CommandLine::command().localize(&demo_localizer);
    let greet = command
        .get_subcommands()
        .find(|sub| sub.get_name() == "greet")
        .expect("greet subcommand must exist");
    let about = greet
        .get_about()
        .expect("greet about should be localized")
        .to_string();
    assert!(about.contains("friendly greeting"));
}

#[rstest]
fn try_parse_with_localizer_builds_cli(demo_localizer: DemoLocalizer) {
    let args = ["hello-world", "greet"];
    let parsed =
        CommandLine::try_parse_localized(args, &demo_localizer).expect("expected to parse args");
    assert!(matches!(parsed.cli.command, Commands::Greet(_)));
}

#[rstest]
fn try_parse_with_localizer_localises_errors(demo_localizer: DemoLocalizer) {
    let err = CommandLine::try_parse_localized(["hello-world"], &demo_localizer)
        .expect_err("missing subcommand should be reported");
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

#[test]
fn noop_localizer_keeps_stock_error_messages() {
    let baseline = CommandLine::command()
        .try_get_matches_from(["hello-world"])
        .expect_err("default clap parsing should fail without subcommand")
        .to_string();

    let err = CommandLine::try_parse_localized(["hello-world"], &DemoLocalizer::noop())
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
    let mut noop_localized_command = CommandLine::command().localize(&noop_localizer);

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
