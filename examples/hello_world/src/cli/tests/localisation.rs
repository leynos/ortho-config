//! Tests covering the CLI localisation helpers.

use super::super::{CommandLine, Commands};
use crate::localizer::DemoLocalizer;

#[test]
fn command_with_localiser_overrides_copy() {
    let localiser = DemoLocalizer::default();
    let command = CommandLine::command_with_localizer(&localiser);
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

#[test]
fn try_parse_with_localiser_builds_cli() {
    let args = ["hello-world", "greet"];
    let localiser = DemoLocalizer::default();
    let cli =
        CommandLine::try_parse_from_localizer(args, &localiser).expect("expected to parse args");
    assert!(matches!(cli.command, Commands::Greet(_)));
}
