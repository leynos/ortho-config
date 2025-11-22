//! Tests covering the CLI localisation helpers.

use super::super::{CommandLine, Commands, LocalizeCmd};
use crate::localizer::DemoLocalizer;
use clap::CommandFactory;

#[test]
fn command_with_localizer_overrides_copy() {
    let localizer = DemoLocalizer::default();
    let command = CommandLine::command().localize(&localizer);
    let about = command
        .get_about()
        .expect("about text should be set")
        .to_string();
    assert!(about.contains("localized"), "expected demo copy in about");

    let long_about = command
        .get_long_about()
        .expect("long about text should be set")
        .to_string();
    assert!(long_about.contains(command.get_name()));
}

#[test]
fn localizes_subcommand_tree() {
    let localizer = DemoLocalizer::default();
    let command = CommandLine::command().localize(&localizer);
    let greet = command
        .get_subcommands()
        .find(|sub| sub.get_name() == "greet")
        .expect("greet subcommand must exist");
    let about = greet
        .get_about()
        .expect("greet about should be localised")
        .to_string();
    assert!(about.contains("friendly greeting"));
}

#[test]
fn try_parse_with_localizer_builds_cli() {
    let args = ["hello-world", "greet"];
    let localizer = DemoLocalizer::default();
    let cli = CommandLine::try_parse_localized(args, &localizer).expect("expected to parse args");
    assert!(matches!(cli.command, Commands::Greet(_)));
}
