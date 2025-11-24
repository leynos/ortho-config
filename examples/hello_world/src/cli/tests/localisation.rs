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
    let cli =
        CommandLine::try_parse_localized(args, &demo_localizer).expect("expected to parse args");
    assert!(matches!(cli.command, Commands::Greet(_)));
}
