//! Integration coverage for localised clap parsing helpers.

use clap::{CommandFactory, Parser};
use ortho_config::{
    LocalizationArgs, LocalizeCmd, LocalizedParse, Localizer, NoOpLocalizer, langid,
    message_id_for, parse_localized_command,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use tracing::Level;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, Registry};
use unic_langid::LanguageIdentifier;

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

struct TranslatedLocalizer {
    locale: LanguageIdentifier,
    messages: BTreeMap<&'static str, &'static str>,
    hits: Mutex<Vec<String>>,
}

impl TranslatedLocalizer {
    fn new(
        locale: LanguageIdentifier,
        messages: impl IntoIterator<Item = (&'static str, &'static str)>,
    ) -> Self {
        Self {
            locale,
            messages: messages.into_iter().collect(),
            hits: Mutex::new(Vec::new()),
        }
    }

    fn fallback(locale: LanguageIdentifier) -> Self {
        Self::new(locale, [])
    }

    fn hits(&self) -> MutexGuard<'_, Vec<String>> {
        match self.hits.lock() {
            Ok(hits) => hits,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn recorded_hits(&self) -> BTreeSet<String> {
        self.hits().iter().cloned().collect()
    }
}

impl Localizer for TranslatedLocalizer {
    fn lookup(&self, id: &str, _args: Option<&LocalizationArgs<'_>>) -> Option<String> {
        self.hits().push(id.to_owned());
        self.messages.get(id).map(ToString::to_string)
    }

    fn locale(&self) -> Option<&LanguageIdentifier> {
        Some(&self.locale)
    }
}

#[derive(Debug, Default)]
struct CapturedEvent {
    level: Option<Level>,
    fields: BTreeMap<String, String>,
}

#[derive(Default)]
struct CapturedEvents {
    events: Mutex<Vec<CapturedEvent>>,
}

impl CapturedEvents {
    fn events(&self) -> MutexGuard<'_, Vec<CapturedEvent>> {
        match self.events.lock() {
            Ok(events) => events,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

struct CaptureLayer {
    events: Arc<CapturedEvents>,
}

impl<S> Layer<S> for CaptureLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut captured = CapturedEvent {
            level: Some(*event.metadata().level()),
            ..CapturedEvent::default()
        };
        event.record(&mut FieldVisitor {
            fields: &mut captured.fields,
        });
        self.events.events().push(captured);
    }
}

struct FieldVisitor<'fields> {
    fields: &'fields mut BTreeMap<String, String>,
}

impl Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}

fn capture_events_during<R>(events: Arc<CapturedEvents>, f: impl FnOnce() -> R) -> R {
    let subscriber = Registry::default().with(CaptureLayer { events });
    tracing::subscriber::with_default(subscriber, f)
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
fn parse_localized_command_uses_translated_metadata_on_success() {
    let localizer = TranslatedLocalizer::new(
        langid!("en-US"),
        [
            ("custom-fixture-about", "Translated fixture help"),
            ("custom-fixture-args-config-value_name", "SETTINGS"),
            ("custom-fixture-greet-about", "Translated greet help"),
            ("custom-fixture-greet-args-name-value_name", "RECIPIENT"),
        ],
    );
    let command = Fixture::command()
        .with_base("custom.fixture")
        .localize(&localizer);
    let (parsed, matches) = parse_localized_command::<Fixture, _, _>(
        command,
        [
            "fixture",
            "--config",
            "settings.toml",
            "greet",
            "--name",
            "Ada",
        ],
        &localizer,
    )
    .expect("translated fixture args should parse");

    assert_eq!(parsed.config, Some(PathBuf::from("settings.toml")));
    assert_eq!(
        parsed.command,
        FixtureCommand::Greet(GreetArgs {
            name: Some("Ada".to_owned())
        })
    );
    assert_eq!(
        matches
            .get_one::<PathBuf>("config")
            .map(std::path::PathBuf::as_path),
        Some(PathBuf::from("settings.toml").as_path())
    );
    assert_eq!(
        localizer.recorded_hits(),
        [
            "custom-fixture-about",
            "custom-fixture-after_help",
            "custom-fixture-after_long_help",
            "custom-fixture-args-config-help",
            "custom-fixture-args-config-long_help",
            "custom-fixture-args-config-value_name",
            "custom-fixture-greet-about",
            "custom-fixture-greet-after_help",
            "custom-fixture-greet-after_long_help",
            "custom-fixture-greet-args-name-help",
            "custom-fixture-greet-args-name-long_help",
            "custom-fixture-greet-args-name-value_name",
            "custom-fixture-greet-long_about",
            "custom-fixture-greet-long_version",
            "custom-fixture-greet-usage",
            "custom-fixture-greet-version",
            "custom-fixture-long_about",
            "custom-fixture-long_version",
            "custom-fixture-usage",
            "custom-fixture-version",
        ]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
    );
}

#[test]
fn noop_localizer_matches_stock_clap() {
    let localized = capture_events_during(Arc::new(CapturedEvents::default()), || {
        Fixture::try_parse_localized_from(["fixture"], &NoOpLocalizer::new())
            .expect_err("missing subcommand should fail")
    });
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
fn missing_clap_error_translation_emits_warning_fields() {
    let events = Arc::new(CapturedEvents::default());
    let localizer = TranslatedLocalizer::fallback(langid!("fr-FR"));

    let err = capture_events_during(Arc::clone(&events), || {
        Fixture::try_parse_localized_from(["fixture"], &localizer)
            .expect_err("missing subcommand should fail")
    });

    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
    let captured_events = events.events();
    let warning = captured_events
        .iter()
        .find(|event| event.level == Some(Level::WARN))
        .expect("missing translation warning should be emitted");
    assert_eq!(
        warning.fields.get("identifier").map(String::as_str),
        Some("clap-error-missing-subcommand")
    );
    assert_eq!(
        warning.fields.get("error_kind").map(String::as_str),
        Some("DisplayHelpOnMissingArgumentOrSubcommand")
    );
    assert_eq!(
        warning.fields.get("locale").map(String::as_str),
        Some("fr-FR")
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
