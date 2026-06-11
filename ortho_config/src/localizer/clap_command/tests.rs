//! Tests for command-tree localization behaviour.

use super::super::{LocalizationArgs, Localizer};
use super::*;
use clap::{Arg, ArgAction, Command};
use rstest::rstest;
use std::collections::HashSet;

struct SelectiveLocalizer {
    ids: HashSet<&'static str>,
}

impl SelectiveLocalizer {
    fn new(ids: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            ids: ids.into_iter().collect(),
        }
    }
}

impl Localizer for SelectiveLocalizer {
    fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        self.ids.contains(id).then(|| format!("{id}:localized"))
    }
}

fn command_tree() -> Command {
    Command::new("demo")
        .about("stock root about")
        .long_about("stock root long about")
        .override_usage("stock root usage")
        .version("1.0.0")
        .long_version("1.0.0 long")
        .after_help("stock root after help")
        .after_long_help("stock root after long help")
        .arg(
            Arg::new("config")
                .long("config")
                .action(ArgAction::Set)
                .help("stock config help")
                .long_help("stock config long help")
                .value_name("FILE"),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .action(ArgAction::SetTrue)
                .help("stock verbose help"),
        )
        .subcommand(Command::new("greet").about("stock greet about"))
}

#[rstest]
fn localize_rewrites_command_tree_without_corrupting_flags() {
    let localizer = SelectiveLocalizer::new([
        "demo-cli-about",
        "demo-cli-long_about",
        "demo-cli-usage",
        "demo-cli-long_version",
        "demo-cli-after_help",
        "demo-cli-after_long_help",
        "demo-cli-args-config-help",
        "demo-cli-args-config-long_help",
        "demo-cli-args-config-value_name",
        "demo-cli-args-verbose-help",
        // This SelectiveLocalizer entry proves flag value-name lookups are ignored.
        "demo-cli-args-verbose-value_name",
        "demo-cli-greet-about",
    ]);

    let mut command = command_tree().with_base("demo.cli").localize(&localizer);

    assert_eq!(
        command.get_about().map(ToString::to_string).as_deref(),
        Some("demo-cli-about:localized")
    );
    assert_eq!(
        command.get_long_about().map(ToString::to_string).as_deref(),
        Some("demo-cli-long_about:localized")
    );
    assert_eq!(command.get_version(), Some("1.0.0"));
    assert_eq!(
        command.get_long_version(),
        Some("demo-cli-long_version:localized")
    );
    assert_eq!(
        command.get_after_help().map(ToString::to_string).as_deref(),
        Some("demo-cli-after_help:localized")
    );
    assert_eq!(
        command
            .get_after_long_help()
            .map(ToString::to_string)
            .as_deref(),
        Some("demo-cli-after_long_help:localized")
    );

    let help = command.render_long_help().to_string();
    assert!(
        help.contains("demo-cli-usage:localized"),
        "localized usage should appear in help:\n{help}"
    );

    let config = command
        .get_arguments()
        .find(|arg| arg.get_id() == "config")
        .expect("config argument should exist");
    assert_eq!(
        config.get_help().map(ToString::to_string).as_deref(),
        Some("demo-cli-args-config-help:localized")
    );
    assert_eq!(
        config.get_long_help().map(ToString::to_string).as_deref(),
        Some("demo-cli-args-config-long_help:localized")
    );
    assert_eq!(
        config
            .get_value_names()
            .and_then(|names| names.first())
            .map(ToString::to_string)
            .as_deref(),
        Some("demo-cli-args-config-value_name:localized")
    );

    let verbose = command
        .get_arguments()
        .find(|arg| arg.get_id() == "verbose")
        .expect("verbose argument should exist");
    assert!(matches!(verbose.get_action(), ArgAction::SetTrue));
    assert!(verbose.get_value_names().is_none());
    assert_eq!(
        verbose.get_help().map(ToString::to_string).as_deref(),
        Some("demo-cli-args-verbose-help:localized")
    );

    let greet = command
        .find_subcommand("greet")
        .expect("greet subcommand should exist");
    assert_eq!(
        greet.get_about().map(ToString::to_string).as_deref(),
        Some("demo-cli-greet-about:localized")
    );
}

#[rstest]
fn localize_self_does_not_recurse_into_subcommands() {
    let localizer = SelectiveLocalizer::new(["demo-cli-about", "demo-cli-greet-about"]);

    let command = command_tree()
        .with_base("demo.cli")
        .localize_self(&localizer);

    assert_eq!(
        command.get_about().map(ToString::to_string).as_deref(),
        Some("demo-cli-about:localized")
    );
    let greet = command
        .find_subcommand("greet")
        .expect("greet subcommand should exist");
    assert_eq!(
        greet.get_about().map(ToString::to_string).as_deref(),
        Some("stock greet about")
    );
}

#[rstest]
#[should_panic(expected = "localized command identifier collision")]
fn localize_panics_on_colliding_subcommand_identifiers() {
    let localizer = SelectiveLocalizer::new([]);
    let command = Command::new("demo")
        .subcommand(Command::new("Foo"))
        .subcommand(Command::new("foo"));

    drop(command.localize(&localizer));
}

#[rstest]
#[should_panic(expected = "localized argument identifier collision")]
fn localize_panics_on_colliding_argument_identifiers() {
    let localizer = SelectiveLocalizer::new([]);
    let command = Command::new("demo")
        .arg(Arg::new("Foo").long("foo"))
        .arg(Arg::new("foo").long("foo2"));

    drop(command.localize(&localizer));
}
