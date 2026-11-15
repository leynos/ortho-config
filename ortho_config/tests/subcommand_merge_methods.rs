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
//! inject, and this suite must not change either in its own process. Each case
//! therefore runs the method in a child process (`support/process_child.rs`)
//! started in a staged temporary directory with every home and platform rung
//! pointed at an empty tree. The environment is exact rather than neutralised,
//! so nothing the host exports can reach the merge.
//!
//! The structs, sentinels, staging helpers, and the row driver live in
//! `support/subcommand_merge_support.rs`, which keeps this file within the
//! repository's module-size limit without trimming coverage to get there.

use anyhow::{Context, Result, bail, ensure};
use clap::{CommandFactory, FromArgMatches};
use ortho_config::SubcmdConfigMerge;
use rstest::rstest;

#[path = "support/process_child.rs"]
mod process_child;
#[path = "support/subcommand_merge_support.rs"]
mod support;

use process_child::ChildEnvironment;
use support::{
    CLAP_DEFAULT_RETRIES, CLI_REFERENCE, CLI_RETRIES, CliMode, FILE_RETRIES, INJECTED_REFERENCE,
    INJECTED_RETRIES, LayerRow, MatchesMethod, PR_REFERENCE_KEY, PrArgs, STRUCT_DEFAULT_RETRIES,
    StagedDir, empty_dir, issue_dir, merged_retries, pr_dir, scan_source,
};

/// The name of the child test in this binary.
const CHILD_TEST: &str = "subcommand_methods_child";

/// Child half of every process-backed case.
///
/// It does nothing unless a parent started it with a case, so it passes
/// trivially in a normal run. The case name selects the assertion; the process
/// environment and working directory it observes are the ones the parent
/// chose.
#[test]
fn subcommand_methods_child() -> Result<()> {
    let Some(case) = process_child::case() else {
        return Ok(());
    };
    if let Some(encoded) = case.strip_prefix("row:") {
        return assert_row(LayerRow::decode(encoded)?);
    }
    match case.as_str() {
        "bare" => assert_bare_home_is_excluded(),
        "pr_sources_beat_file" => assert_sources_beat_file(),
        "pr_sources_beneath_cli" => assert_sources_beneath_cli(),
        other => bail!("unknown child case `{other}`"),
    }
}

/// Run `case` in a child whose working directory is `staged`'s.
fn run_in(staged: &StagedDir, case: &str) -> Result<()> {
    process_child::run(
        CHILD_TEST,
        case,
        &ChildEnvironment {
            dir: staged.dir.path(),
            empty_root: staged.empty_root.path(),
            vars: &[],
        },
    )
}

/// Child assertion: nothing but the exact environment reaches the merge.
fn assert_bare_home_is_excluded() -> Result<()> {
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

/// Child assertion: the injected source beats the file value.
fn assert_sources_beat_file() -> Result<()> {
    let merged = PrArgs::default()
        .load_and_merge_with_sources(scan_source(PR_REFERENCE_KEY, INJECTED_REFERENCE))
        .context("merge pr args from the injected source")?;
    ensure!(
        merged.reference.as_deref() == Some(INJECTED_REFERENCE),
        "expected {INJECTED_REFERENCE:?} above the file value, got {:?}",
        merged.reference
    );
    Ok(())
}

/// Child assertion: an explicit CLI value stays above the injected source.
fn assert_sources_beneath_cli() -> Result<()> {
    let matches = PrArgs::command().get_matches_from(["pr", "--reference", CLI_REFERENCE]);
    let cli = PrArgs::from_arg_matches(&matches).context("parse explicit values")?;
    let merged = cli
        .load_and_merge_with_sources(scan_source(PR_REFERENCE_KEY, INJECTED_REFERENCE))
        .context("merge pr args carrying CLI values")?;
    ensure!(
        merged.reference.as_deref() == Some(CLI_REFERENCE),
        "expected the CLI value above the injected source, got {:?}",
        merged.reference
    );
    Ok(())
}

/// Child assertion: one table row merges to its expected retry count.
fn assert_row(row: LayerRow) -> Result<()> {
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

/// Discovery resolves the home and XDG rungs through the process environment
/// rather than the working directory, so entering a temporary directory does
/// not close them.
///
/// The child runs with an exact environment, so a developer's own
/// `~/.vk.toml` or `~/.config/vk/config.toml` cannot be merged into the
/// result. This pins that invariant: were the harness ever reduced to a
/// working-directory change, every other case in this module would once again
/// depend on what the host has installed, and nothing else here would catch it.
#[test]
fn child_environment_excludes_the_ambient_home() -> Result<()> {
    run_in(&empty_dir()?, "bare")
}

/// `SubcmdConfigMerge::load_and_merge_with_sources` merges the supplied scan
/// source as the environment layer, above the file but below the CLI.
///
/// Catches a forwarding that drops the supplied source and reads the process
/// environment instead, which would leave `FILE_REFERENCE` in place. The child's
/// environment is exact, so a `PR_REFERENCE_KEY` exported by the host cannot
/// satisfy the assertion without the injected source being read at all.
#[test]
fn sources_method_prefers_the_injected_environment_over_the_file() -> Result<()> {
    run_in(&pr_dir()?, "pr_sources_beat_file")
}

/// `SubcmdConfigMerge::load_and_merge_with_sources` keeps explicit CLI values
/// above the injected environment.
///
/// Catches a forwarding that omits the CLI overlay, which would leave
/// `INJECTED_REFERENCE` in place instead of `CLI_REFERENCE`.
#[test]
fn sources_method_keeps_cli_values_above_the_injected_environment() -> Result<()> {
    run_in(&pr_dir()?, "pr_sources_beneath_cli")
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
fn matches_methods_place_each_layer_without_a_file(#[case] row: LayerRow) -> Result<()> {
    run_in(&empty_dir()?, &format!("row:{}", row.encode()))
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
/// The child's environment is exact, so a value that is present came from the
/// injected source rather than the host.
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
fn matches_methods_place_each_layer_over_a_file(#[case] row: LayerRow) -> Result<()> {
    run_in(&issue_dir()?, &format!("row:{}", row.encode()))
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
