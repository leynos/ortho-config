//! Integration coverage for localised clap parsing helpers.

use clap::{CommandFactory, Parser};
use ortho_config::{
    LocalizationArgs, LocalizeCmd, LocalizedParse, Localizer, NoOpLocalizer, message_id_for,
    parse_localized_command,
};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

#[derive(Debug, Parser)]
#[command(name = "fixture", bin_name = "fixture")]
struct Fixture {
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: FixtureCommand,
}

#[derive(Debug, PartialEq, Eq, clap::Subcommand)]
enum FixtureCommand {
    Greet(GreetArgs),
}

#[derive(Debug, PartialEq, Eq, clap::Args)]
struct GreetArgs {
    #[arg(long, value_name = "NAME")]
    name: Option<String>,
}

#[derive(Debug, Parser)]
#[command(name = "123-fixture", bin_name = "123-fixture")]
struct UnsafeFixture {
    #[arg(long, id = "bad.id")]
    bad: Option<String>,
}

#[derive(Default)]
struct RecordingLocalizer {
    ids: Mutex<Vec<String>>,
}

impl RecordingLocalizer {
    fn ids(&self) -> MutexGuard<'_, Vec<String>> {
        match self.ids.lock() {
            Ok(ids) => ids,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn recorded_ids(&self) -> BTreeSet<String> {
        self.ids().iter().cloned().collect()
    }
}

impl Localizer for RecordingLocalizer {
    fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        self.ids().push(id.to_owned());
        None
    }
}

struct MissingSubcommandLocalizer;

impl Localizer for MissingSubcommandLocalizer {
    fn lookup(&self, id: &str, args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        if id != "clap-error-missing-subcommand" {
            return None;
        }

        let valid_subcommands = args
            .and_then(|localization_args| localization_args.get("valid_subcommands"))
            .map_or_else(|| "<missing>".to_owned(), |value| format!("{value:?}"));

        Some(format!("choose one of: {valid_subcommands}"))
    }
}

#[test]
fn try_parse_localized_from_parses_subcommand() {
    let parsed = Fixture::try_parse_localized_from(["fixture", "greet"], &NoOpLocalizer::new())
        .expect("fixture args should parse");

    assert_eq!(
        parsed.command,
        FixtureCommand::Greet(GreetArgs { name: None })
    );
}

#[test]
fn try_parse_localized_with_matches_returns_matches() {
    let (_parsed, matches) =
        Fixture::try_parse_localized_with_matches(["fixture", "greet"], &NoOpLocalizer::new())
            .expect("fixture args should parse");

    assert_eq!(matches.subcommand_name(), Some("greet"));
}

#[test]
fn noop_localizer_matches_stock_clap() {
    let localized = Fixture::try_parse_localized_from(["fixture"], &NoOpLocalizer::new())
        .expect_err("missing subcommand should fail");
    let stock = Fixture::command()
        .try_get_matches_from(["fixture"])
        .expect_err("stock clap should reject missing subcommand");

    assert_eq!(localized.to_string(), stock.to_string());
}

#[test]
fn from_arg_matches_error_retains_valid_subcommands() {
    let mut command = Fixture::command();
    command = command.subcommand_required(false);
    let err = parse_localized_command::<Fixture, _, _>(
        command.localize(&NoOpLocalizer::new()),
        ["fixture"],
        &MissingSubcommandLocalizer,
    )
    .expect_err("from_arg_matches should reject the missing subcommand");

    assert!(
        err.to_string().contains("greet"),
        "localized error should list valid subcommands: {err}"
    );
}

#[test]
fn identifier_coverage_matches_message_id_for() {
    let localizer = RecordingLocalizer::default();
    drop(Fixture::command().localize(&localizer));

    let expected = [
        message_id_for(&["fixture"], "about"),
        message_id_for(&["fixture"], "long_about"),
        message_id_for(&["fixture"], "usage"),
        message_id_for(&["fixture"], "version"),
        message_id_for(&["fixture"], "long_version"),
        message_id_for(&["fixture"], "after_help"),
        message_id_for(&["fixture"], "after_long_help"),
        message_id_for(&["fixture"], "args.config.help"),
        message_id_for(&["fixture"], "args.config.long_help"),
        message_id_for(&["fixture"], "args.config.value_name"),
        message_id_for(&["fixture", "greet"], "about"),
        message_id_for(&["fixture", "greet"], "long_about"),
        message_id_for(&["fixture", "greet"], "usage"),
        message_id_for(&["fixture", "greet"], "version"),
        message_id_for(&["fixture", "greet"], "long_version"),
        message_id_for(&["fixture", "greet"], "after_help"),
        message_id_for(&["fixture", "greet"], "after_long_help"),
        message_id_for(&["fixture", "greet"], "args.name.help"),
        message_id_for(&["fixture", "greet"], "args.name.long_help"),
        message_id_for(&["fixture", "greet"], "args.name.value_name"),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();

    assert_eq!(localizer.recorded_ids(), expected);
}

#[test]
#[should_panic(expected = "Fluent identifier must start with an ASCII letter")]
fn fluent_unsafe_identifier_panics() {
    drop(UnsafeFixture::try_parse_localized_from(
        ["123-fixture", "--bad", "value"],
        &NoOpLocalizer::new(),
    ));
}
