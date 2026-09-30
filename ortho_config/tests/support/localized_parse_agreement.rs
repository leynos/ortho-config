//! Cross-crate agreement between derived localization constants and the
//! runtime walker.
//!
//! These cases lock the identifiers the derive emits against the ones
//! `message_id_for` builds at runtime. A drift between the two
//! derivations fails here rather than silently orphaning a catalogue
//! entry that no lookup ever requests.

use super::*;

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
#[derive(Debug, PartialEq, Parser, Deserialize, Serialize, OrthoConfig)]
#[command(name = "tree", bin_name = "tree")]
#[ortho_config(localization_base = "treecli")]
struct TreeCli {
    #[arg(long)]
    config: Option<String>,
    #[serde(skip)]
    #[command(subcommand)]
    command: TreeCommand,
    #[command(flatten)]
    #[ortho_config(skip_cli)]
    extra: ExtraArgs,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Subcommand, OrthoConfigSubcommandDocs)]
enum TreeCommand {
    Greet(TreeGreetArgs),
}

impl Default for TreeCommand {
    fn default() -> Self {
        Self::Greet(TreeGreetArgs::default())
    }
}

#[derive(Debug, Default, PartialEq, Args, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "TREE")]
struct TreeGreetArgs {
    #[arg(long)]
    name: Option<String>,
}

/// The flattened group type. The runtime mounts flattened fields under the
/// parent command's `args.` namespace; the derived parent surface's `ARG_IDS`
/// deliberately excludes them via `clap_field_is_flattened` (D-12).
#[derive(Debug, Default, PartialEq, Args, Deserialize, Serialize)]
struct ExtraArgs {
    #[arg(long)]
    extra: Option<String>,
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
