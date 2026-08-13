//! Integration coverage for localised clap parsing helpers.

use clap::{CommandFactory, Parser};
use ortho_config::{
    ArgLocalizationIds, LocalizationArgs, LocalizeCmd, LocalizedParse, Localizer, NoOpLocalizer,
    OrthoConfig, OrthoConfigLocalization, langid, message_id_for, parse_localized_command,
};
use rstest::{fixture, rstest};
use serde::{Deserialize, Serialize};
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

struct CaptureLayer(Arc<CapturedEvents>);

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
        self.0.events().push(captured);
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
    let subscriber = Registry::default().with(CaptureLayer(events));
    tracing::subscriber::with_default(subscriber, f)
}

#[fixture]
fn missing_subcommand_localizer() -> MissingSubcommandLocalizer {
    MissingSubcommandLocalizer
}

#[fixture]
fn fallback_localizer() -> TranslatedLocalizer {
    TranslatedLocalizer::new(langid!("fr-FR"), [])
}

#[fixture]
fn translated_localizer() -> TranslatedLocalizer {
    TranslatedLocalizer::new(
        langid!("en-US"),
        [
            ("custom-fixture-about", "Translated fixture help"),
            ("custom-fixture-args-config-value_name", "SETTINGS"),
            ("custom-fixture-greet-about", "Translated greet help"),
            ("custom-fixture-greet-args-name-value_name", "RECIPIENT"),
        ],
    )
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

#[rstest]
fn parse_localized_command_uses_translated_metadata_on_success(
    translated_localizer: TranslatedLocalizer,
) {
    let command = Fixture::command()
        .with_base("custom.fixture")
        .localize(&translated_localizer);
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
        &translated_localizer,
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
        translated_localizer.recorded_hits(),
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

#[rstest]
fn from_arg_matches_error_retains_valid_subcommands(
    missing_subcommand_localizer: MissingSubcommandLocalizer,
) {
    let mut command = Fixture::command();
    command = command.subcommand_required(false);
    let err = parse_localized_command::<Fixture, _, _>(
        command.localize(&NoOpLocalizer::new()),
        ["fixture"],
        &missing_subcommand_localizer,
    )
    .expect_err("from_arg_matches should reject the missing subcommand");

    assert!(
        err.to_string().contains("greet"),
        "localized error should list valid subcommands: {err}"
    );
}

#[rstest]
fn missing_clap_error_translation_emits_warning_fields(fallback_localizer: TranslatedLocalizer) {
    let events = Arc::new(CapturedEvents::default());

    let err = capture_events_during(Arc::clone(&events), || {
        Fixture::try_parse_localized_from(["fixture"], &fallback_localizer)
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

/// Flat derived CLI (no subcommands, no flatten) used for the cross-crate
/// agreement lock: its derived `OrthoConfigLocalization` constants must equal
/// the runtime walker's recorded identifiers exactly.
#[derive(Debug, PartialEq, Parser, Serialize, Deserialize, OrthoConfig)]
#[command(name = "flat", bin_name = "flat")]
#[ortho_config(localization_base = "flatcli")]
struct FlatCli {
    #[arg(long)]
    config: Option<String>,
    #[arg(long)]
    verbose: Option<String>,
}

/// Derived CLI with a subcommand and a flattened group. The parent surface's
/// `ARG_IDS` excludes the subcommand selector and the flattened field; the
/// runtime walker still records subcommand-node and flattened-argument ids,
/// which the test accounts for as a documented remainder (D-9, D-12).
#[derive(Debug, PartialEq, Parser)]
#[command(name = "tree", bin_name = "tree")]
struct TreeCli {
    #[arg(long)]
    config: Option<String>,
    #[command(subcommand)]
    command: TreeCommand,
    #[command(flatten)]
    extra: ExtraArgs,
}

#[derive(Debug, PartialEq, clap::Subcommand)]
enum TreeCommand {
    Greet(TreeGreetArgs),
}

#[derive(Debug, PartialEq, clap::Args)]
struct TreeGreetArgs {
    #[arg(long)]
    name: Option<String>,
}

/// The flattened group type. The runtime mounts flattened fields under the
/// parent command's `args.` namespace; the parent surface's `ARG_IDS` (here a
/// handwritten `OrthoConfigLocalization` impl) deliberately excludes them
/// (D-12).
#[derive(Debug, PartialEq, clap::Args)]
struct ExtraArgs {
    #[arg(long)]
    extra: Option<String>,
}

/// Handwritten `OrthoConfigLocalization` for `TreeCli`, modelling what the
/// derive would emit for a parent surface: `ARG_IDS` covers the parent's own
/// fields (`config`) and excludes the flattened group (`extra`) and the
/// subcommand selector (D-9, D-12).
impl OrthoConfigLocalization for TreeCli {
    const LOCALIZATION_BASE: &'static str = "treecli";
    const ABOUT_ID: &'static str = "treecli-about";
    const LONG_ABOUT_ID: &'static str = "treecli-long_about";
    const USAGE_ID: &'static str = "treecli-usage";
    const VERSION_ID: &'static str = "treecli-version";
    const LONG_VERSION_ID: &'static str = "treecli-long_version";
    const AFTER_HELP_ID: &'static str = "treecli-after_help";
    const AFTER_LONG_HELP_ID: &'static str = "treecli-after_long_help";
    const ARG_IDS: &'static [ArgLocalizationIds] = &[ArgLocalizationIds {
        name: "config",
        help_id: "treecli-args-config-help",
        long_help_id: "treecli-args-config-long_help",
        value_name_id: "treecli-args-config-value_name",
    }];
}

/// Collects every command-level constant plus the `ARG_IDS` entries of a
/// derived surface into one set.
fn derived_constant_set<T: OrthoConfigLocalization>() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    set.insert(T::ABOUT_ID.to_owned());
    set.insert(T::LONG_ABOUT_ID.to_owned());
    set.insert(T::USAGE_ID.to_owned());
    set.insert(T::VERSION_ID.to_owned());
    set.insert(T::LONG_VERSION_ID.to_owned());
    set.insert(T::AFTER_HELP_ID.to_owned());
    set.insert(T::AFTER_LONG_HELP_ID.to_owned());
    for arg in T::ARG_IDS {
        set.insert(arg.help_id.to_owned());
        set.insert(arg.long_help_id.to_owned());
        set.insert(arg.value_name_id.to_owned());
    }
    set
}

/// Looks up an `ARG_IDS` entry by clap id and asserts its three identifiers
/// equal the §4.1 messages the runtime walker would request.
fn assert_arg_ids_match_message_id_for(arg: &ArgLocalizationIds, base: &[&str]) {
    assert_eq!(
        arg.help_id,
        message_id_for(base, &format!("args.{}.help", arg.name))
    );
    assert_eq!(
        arg.long_help_id,
        message_id_for(base, &format!("args.{}.long_help", arg.name))
    );
    assert_eq!(
        arg.value_name_id,
        message_id_for(base, &format!("args.{}.value_name", arg.name))
    );
}

#[test]
fn flat_command_constants_equal_message_id_for() {
    let base = FlatCli::LOCALIZATION_BASE.split('.').collect::<Vec<_>>();
    assert_eq!(FlatCli::ABOUT_ID, message_id_for(&base, "about"));
    assert_eq!(FlatCli::LONG_ABOUT_ID, message_id_for(&base, "long_about"));
    assert_eq!(FlatCli::USAGE_ID, message_id_for(&base, "usage"));
    assert_eq!(FlatCli::VERSION_ID, message_id_for(&base, "version"));
    assert_eq!(
        FlatCli::LONG_VERSION_ID,
        message_id_for(&base, "long_version")
    );
    assert_eq!(FlatCli::AFTER_HELP_ID, message_id_for(&base, "after_help"));
    assert_eq!(
        FlatCli::AFTER_LONG_HELP_ID,
        message_id_for(&base, "after_long_help")
    );
}

#[test]
fn flat_argument_constants_equal_message_id_for() {
    let base = FlatCli::LOCALIZATION_BASE.split('.').collect::<Vec<_>>();
    let config = FlatCli::ARG_IDS
        .iter()
        .find(|arg| arg.name == "config")
        .expect("flat fixture should carry a config argument");
    assert_arg_ids_match_message_id_for(config, &base);
    let verbose = FlatCli::ARG_IDS
        .iter()
        .find(|arg| arg.name == "verbose")
        .expect("flat fixture should carry a verbose argument");
    assert_arg_ids_match_message_id_for(verbose, &base);
}

#[test]
fn flat_walker_coverage_equals_derived_constants() {
    let localizer = RecordingLocalizer::default();
    drop(
        FlatCli::command()
            .with_base(FlatCli::LOCALIZATION_BASE)
            .localize(&localizer),
    );

    let expected = derived_constant_set::<FlatCli>();
    assert_eq!(localizer.recorded_ids(), expected);
}

#[test]
fn subcommand_flatten_constants_are_subset_with_documented_remainder() {
    let localizer = RecordingLocalizer::default();
    drop(
        TreeCli::command()
            .with_base(TreeCli::LOCALIZATION_BASE)
            .localize(&localizer),
    );

    let constants = derived_constant_set::<TreeCli>();
    let recorded = localizer.recorded_ids();
    assert!(
        constants.is_subset(&recorded),
        "derived constants must be a subset of the runtime walker coverage"
    );

    let remainder = recorded
        .difference(&constants)
        .cloned()
        .collect::<BTreeSet<_>>();
    let base = TreeCli::LOCALIZATION_BASE.split('.').collect::<Vec<_>>();
    let mut expected_remainder = BTreeSet::new();
    // Subcommand node (D-9): every command-level suffix plus its argument ids.
    for suffix in [
        "about",
        "long_about",
        "usage",
        "version",
        "long_version",
        "after_help",
        "after_long_help",
    ] {
        expected_remainder.insert(message_id_for(&["treecli", "greet"], suffix));
    }
    for suffix in ["help", "long_help", "value_name"] {
        expected_remainder.insert(message_id_for(
            &["treecli", "greet"],
            &format!("args.name.{suffix}"),
        ));
    }
    // Flattened argument (D-12): mounted under the parent command's `args.`.
    for suffix in ["help", "long_help", "value_name"] {
        expected_remainder.insert(message_id_for(&base, &format!("args.extra.{suffix}")));
    }

    assert_eq!(remainder, expected_remainder);
}
