//! Behavioural coverage for the `SubcmdConfigMerge` method family.
//!
//! Every case drives one public trait method and stages its sources so that a
//! mis-wired forwarding fails rather than passing by accident: an injected
//! environment value must beat a file value, an explicit CLI value must beat
//! the injected environment, and a field marked
//! `#[ortho_config(cli_default_as_absent)]` must stay absent when clap only
//! supplied its own default.
//!
//! The two match-aware methods are driven from one table, because they differ
//! only in where their environment layer comes from — the process environment,
//! or a caller-supplied scan source — and a row names which of the two it
//! exercises. `merged_retries` performs the parse-and-merge the rows share, so
//! each row carries only what distinguishes it: the method, the CLI shape, the
//! injected value, and the layer that should win. The expectations are distinct
//! per row, so a table row can still fail only for its own reason.
//!
//! The methods are process-backed: they resolve configuration files through
//! `ProcessEnv` and the working directory, neither of which a caller can
//! inject, so isolation has to be arranged around them rather than supplied to
//! them. Each case enters a temporary directory of its own *and* points every
//! environment rung discovery consults into an empty temporary tree.
//!
//! Working-directory isolation alone would not be enough. `collect_unix_paths`
//! and `collect_non_unix_paths` push the home and platform candidates *before*
//! the local ones, so a developer's own `~/.vk.toml` or
//! `~/.config/vk/config.toml` would otherwise be merged into the result.
//!
//! The structs, sentinels, fixtures, and the row driver live in
//! `support/subcommand_merge_support.rs`, which keeps this file within the
//! repository's module-size limit without trimming coverage to get there.

use anyhow::{Context, Result, bail, ensure};
use clap::{CommandFactory, FromArgMatches};
use ortho_config::SubcmdConfigMerge;
use rstest::rstest;
use test_helpers::cwd;

#[path = "support/subcommand_merge_support.rs"]
mod support;

use support::{
    CLAP_DEFAULT_RETRIES, CLI_REFERENCE, CLI_RETRIES, CliMode, ConfigDir, FILE_RETRIES,
    INJECTED_REFERENCE, INJECTED_RETRIES, ISSUE_RETRIES_KEY, IsolatedEnv, LayerRow, MatchesMethod,
    PR_REFERENCE_KEY, PrArgs, STRUCT_DEFAULT_RETRIES, empty_dir, ensure_key_is_unset, isolated_env,
    issue_dir, merged_retries, pr_dir, scan_source,
};

/// Discovery resolves the home and XDG rungs through the process environment
/// rather than the working directory, so entering a temporary directory does
/// not close them.
///
/// This pins the invariant [`isolated_env`] exists to maintain. Were the
/// fixture ever reduced to a working-directory change, every other case in this
/// module would once again merge a developer's own `~/.vk.toml` or
/// `~/.config/vk/config.toml`, and nothing else here would catch that: the suite
/// still passes on a host that has neither file.
#[rstest]
fn isolated_env_excludes_the_ambient_home(isolated_env: Result<IsolatedEnv>) -> Result<()> {
    let _isolated = isolated_env?;
    ensure_key_is_unset(PR_REFERENCE_KEY)?;
    let bare = tempfile::tempdir().context("create bare working directory")?;
    let _guard = cwd::set_dir(bare.path()).context("enter bare working directory")?;

    // Nothing but the process environment is configured, and the fixture has
    // emptied it of every rung discovery consults.
    let merged = PrArgs::default()
        .load_and_merge()
        .context("merge against the isolated environment")?;

    ensure!(
        merged.reference.is_none(),
        "an ambient home or XDG file reached the merge: {:?}",
        merged.reference
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_sources` merges the supplied scan
/// source as the environment layer, above the file but below the CLI.
///
/// Catches a forwarding that drops the supplied source and reads the process
/// environment instead, which would leave `FILE_REFERENCE` in place. The
/// precondition keeps the case non-vacuous: a `PR_REFERENCE_KEY` exported into
/// the process environment could otherwise satisfy the assertion without the
/// injected source being read at all.
#[rstest]
fn sources_method_prefers_the_injected_environment_over_the_file(
    isolated_env: Result<IsolatedEnv>,
    pr_dir: Result<ConfigDir>,
) -> Result<()> {
    let _isolated = isolated_env?;
    let _dir = pr_dir?;
    ensure_key_is_unset(PR_REFERENCE_KEY)?;
    let source = scan_source(PR_REFERENCE_KEY, INJECTED_REFERENCE);

    let cli = PrArgs::default();
    let merged = cli
        .load_and_merge_with_sources(source)
        .context("merge pr args from the injected source")?;

    ensure!(
        merged.reference.as_deref() == Some(INJECTED_REFERENCE),
        "expected {INJECTED_REFERENCE:?} above the file value, got {:?}",
        merged.reference
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_sources` keeps explicit CLI values
/// above the injected environment.
///
/// Catches a forwarding that omits the CLI overlay, which would leave
/// `INJECTED_REFERENCE` in place instead of `CLI_REFERENCE`.
#[rstest]
fn sources_method_keeps_cli_values_above_the_injected_environment(
    isolated_env: Result<IsolatedEnv>,
    pr_dir: Result<ConfigDir>,
) -> Result<()> {
    let _isolated = isolated_env?;
    let _dir = pr_dir?;
    let source = scan_source(PR_REFERENCE_KEY, INJECTED_REFERENCE);
    let matches = PrArgs::command().get_matches_from(["pr", "--reference", CLI_REFERENCE]);
    let cli = PrArgs::from_arg_matches(&matches).context("parse explicit values")?;

    let merged = cli
        .load_and_merge_with_sources(source)
        .context("merge pr args carrying CLI values")?;

    ensure!(
        merged.reference.as_deref() == Some(CLI_REFERENCE),
        "expected the CLI value above the injected source, got {:?}",
        merged.reference
    );
    Ok(())
}

/// The match-aware methods place every layer on the right rung when no
/// configuration file is present.
///
/// Cases:
///
/// - `clap_default_stays_absent` catches a forwarding that serialises the
///   parsed values wholesale, leaking `CLAP_DEFAULT_RETRIES` over the explicit
///   struct default. Only the empty directory can show this: with a file
///   present its value would answer first.
/// - `injected_source_beats_empty_file` catches a forwarding that drops the
///   supplied source, leaving the (absent) file layer in place.
/// - `explicit_cli_value_wins` catches one that drops the parsed CLI values or
///   substitutes the struct default.
/// - `injected_source_beneath_cli` catches one that applies the environment
///   layer last, leaving `INJECTED_RETRIES` instead of `CLI_RETRIES`.
#[rstest]
#[case::clap_default_stays_absent(LayerRow::new(
    MatchesMethod::Plain,
    CliMode::Absent,
    None,
    STRUCT_DEFAULT_RETRIES
))]
#[case::injected_source_beats_empty_file(LayerRow::new(
    MatchesMethod::WithSources,
    CliMode::Absent,
    Some(INJECTED_RETRIES),
    INJECTED_RETRIES
))]
#[case::explicit_cli_value_wins(LayerRow::new(
    MatchesMethod::Plain,
    CliMode::Explicit,
    None,
    CLI_RETRIES
))]
#[case::injected_source_beneath_cli(LayerRow::new(
    MatchesMethod::WithSources,
    CliMode::Explicit,
    Some(INJECTED_RETRIES),
    CLI_RETRIES
))]
fn matches_methods_place_each_layer_without_a_file(
    isolated_env: Result<IsolatedEnv>,
    empty_dir: Result<ConfigDir>,
    #[case] row: LayerRow,
) -> Result<()> {
    assert_layer_rung(isolated_env, empty_dir, row)
}

/// The match-aware methods place every layer on the right rung when a
/// configuration file is present.
///
/// Cases:
///
/// - `file_beats_clap_default` catches a forwarding that ignores `matches`;
///   reaching the file at all also shows that file discovery still runs.
/// - `injected_source_beats_file` catches one that drops the supplied source,
///   leaving `FILE_RETRIES`. Staging a distinct file value also catches a
///   precedence inversion that would let the file win.
/// - `explicit_cli_value_beats_file_and_source` catches one that lets either
///   lower layer win over the CLI.
///
/// Every case also requires `ISSUE_RETRIES_KEY` to be unset in the process
/// environment, so a value that is present came from the injected source rather
/// than the live environment.
#[rstest]
#[case::file_beats_clap_default(LayerRow::new(
    MatchesMethod::Plain,
    CliMode::Absent,
    Some(INJECTED_RETRIES),
    FILE_RETRIES
))]
#[case::injected_source_beats_file(LayerRow::new(
    MatchesMethod::WithSources,
    CliMode::Absent,
    Some(INJECTED_RETRIES),
    INJECTED_RETRIES
))]
#[case::explicit_cli_value_beats_file_and_source(LayerRow::new(
    MatchesMethod::WithSources,
    CliMode::Explicit,
    Some(INJECTED_RETRIES),
    CLI_RETRIES
))]
fn matches_methods_place_each_layer_over_a_file(
    isolated_env: Result<IsolatedEnv>,
    issue_dir: Result<ConfigDir>,
    #[case] row: LayerRow,
) -> Result<()> {
    assert_layer_rung(isolated_env, issue_dir, row)
}

/// Drives one table row against the configuration directory the case entered.
///
/// The directory arrives as a consumed fixture rather than being chosen from a
/// row field: staging is a case-level decision, so which directory a row runs
/// against is visible in the test that owns it, and the only expected value a
/// row can carry is one its own layers can produce.
fn assert_layer_rung(
    isolated_env: Result<IsolatedEnv>,
    staged: Result<ConfigDir>,
    row: LayerRow,
) -> Result<()> {
    let _isolated = isolated_env?;
    let _dir = staged?;
    ensure_key_is_unset(ISSUE_RETRIES_KEY)?;

    let merged = merged_retries(row.method, row.cli, row.injected)?;

    ensure!(
        merged == row.expected,
        "{:?} with {:?} CLI and injected {:?}: expected {}, got {merged} — the clap default \
         {CLAP_DEFAULT_RETRIES} leaking is the usual cause",
        row.method,
        row.cli,
        row.injected,
        row.expected
    );
    Ok(())
}

/// The row driver refuses a `with_sources` row that supplies no injected value.
///
/// Such a row would leave the method with the process environment as its only
/// environment layer, so it would pass or fail exactly like a `Plain` row and
/// could not detect a forwarding that dropped the supplied source. Failing
/// loudly keeps a mis-specified row from looking like coverage.
#[rstest]
fn with_sources_rows_require_an_injected_value() -> Result<()> {
    let Err(err) = merged_retries(MatchesMethod::WithSources, CliMode::Absent, None) else {
        bail!("a with_sources row without an injected value must be refused");
    };

    ensure!(
        err.to_string().contains("must supply an injected value"),
        "refused for the wrong reason: {err}"
    );
    Ok(())
}
