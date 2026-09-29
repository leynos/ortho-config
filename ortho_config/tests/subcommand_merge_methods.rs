//! Behavioural coverage for the `SubcmdConfigMerge` method family.
//!
//! Every case drives one public trait method and stages its sources so that a
//! mis-wired forwarding fails rather than passing by accident: an injected
//! environment value must beat a file value, an explicit CLI value must beat
//! the injected environment, and a field marked
//! `#[ortho_config(cli_default_as_absent)]` must stay absent when clap only
//! supplied its own default.
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
//! The structs, sentinels, and fixtures live in
//! `support/subcommand_merge_support.rs`, which keeps this file within the
//! repository's module-size limit without trimming coverage to get there.

use anyhow::{Context, Result, ensure};
use clap::{CommandFactory, FromArgMatches};
use ortho_config::SubcmdConfigMerge;
use rstest::rstest;
use test_helpers::cwd;

#[path = "support/subcommand_merge_support.rs"]
mod support;

use support::{
    CLAP_DEFAULT_RETRIES, CLI_REFERENCE, CLI_RETRIES, ConfigDir, FILE_RETRIES, INJECTED_REFERENCE,
    INJECTED_RETRIES, ISSUE_RETRIES_KEY, IsolatedEnv, IssueArgs, PR_REFERENCE_KEY, PrArgs,
    STRUCT_DEFAULT_RETRIES, empty_dir, ensure_key_is_unset, isolated_env, issue_dir, pr_dir,
    scan_source,
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

/// `SubcmdConfigMerge::load_and_merge_with_matches` treats a clap default as
/// absent when no match metadata names it explicit.
///
/// Catches a forwarding that ignores `matches` and serialises the parsed values
/// wholesale, which would leak `CLAP_DEFAULT_RETRIES` over the explicit default
/// of `STRUCT_DEFAULT_RETRIES`.
#[rstest]
fn matches_method_keeps_clap_defaults_absent(
    isolated_env: Result<IsolatedEnv>,
    empty_dir: Result<ConfigDir>,
) -> Result<()> {
    let _isolated = isolated_env?;
    let _dir = empty_dir?;
    ensure_key_is_unset(ISSUE_RETRIES_KEY)?;
    let matches = IssueArgs::command().get_matches_from(["issue"]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse clap defaults")?;

    let merged = args
        .load_and_merge_with_matches(&matches)
        .context("merge absent clap defaults")?;

    ensure!(
        merged.retries == STRUCT_DEFAULT_RETRIES,
        "expected {STRUCT_DEFAULT_RETRIES}, not the clap default {CLAP_DEFAULT_RETRIES}: {}",
        merged.retries
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches` still honours a value the
/// user supplied explicitly.
///
/// Catches a forwarding that drops the parsed CLI values or substitutes the
/// struct default, either of which would leave `STRUCT_DEFAULT_RETRIES` instead
/// of `CLI_RETRIES`.
#[rstest]
fn matches_method_honours_explicit_cli_values(
    isolated_env: Result<IsolatedEnv>,
    empty_dir: Result<ConfigDir>,
) -> Result<()> {
    let _isolated = isolated_env?;
    let _dir = empty_dir?;
    let retries = CLI_RETRIES.to_string();
    let matches = IssueArgs::command().get_matches_from(["issue", "--retries", retries.as_str()]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse explicit values")?;

    let merged = args
        .load_and_merge_with_matches(&matches)
        .context("merge explicit CLI values")?;

    ensure!(
        merged.retries == CLI_RETRIES,
        "expected the explicit CLI value {CLI_RETRIES}, got {}",
        merged.retries
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches` lets the file layer supply
/// a value the user did not pass, instead of the clap default.
///
/// Catches a forwarding that ignores `matches`, which would leak
/// `CLAP_DEFAULT_RETRIES` over the staged `FILE_RETRIES`; reaching the file at
/// all also shows that file discovery still runs.
#[rstest]
fn matches_method_prefers_the_file_over_absent_clap_defaults(
    isolated_env: Result<IsolatedEnv>,
    issue_dir: Result<ConfigDir>,
) -> Result<()> {
    let _isolated = isolated_env?;
    let _dir = issue_dir?;
    ensure_key_is_unset(ISSUE_RETRIES_KEY)?;
    let matches = IssueArgs::command().get_matches_from(["issue"]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse clap defaults")?;

    let merged = args
        .load_and_merge_with_matches(&matches)
        .context("merge file defaults beneath clap defaults")?;

    ensure!(
        merged.retries == FILE_RETRIES,
        "expected the file value {FILE_RETRIES}, not the clap default {CLAP_DEFAULT_RETRIES}: {}",
        merged.retries
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches_with_sources` merges the
/// supplied scan source above the file and beneath absent clap defaults.
///
/// Catches a forwarding that drops the supplied source, which would leave
/// `FILE_RETRIES` in place instead of `INJECTED_RETRIES`. Staging a distinct
/// file value also catches a precedence inversion that would let the file win,
/// and requiring the key to be unset in the process environment shows the value
/// came from the injected source rather than from the live environment.
#[rstest]
fn matches_with_sources_method_reads_the_injected_environment(
    isolated_env: Result<IsolatedEnv>,
    issue_dir: Result<ConfigDir>,
) -> Result<()> {
    let _isolated = isolated_env?;
    let _dir = issue_dir?;
    ensure_key_is_unset(ISSUE_RETRIES_KEY)?;
    let matches = IssueArgs::command().get_matches_from(["issue"]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse clap defaults")?;
    let source = scan_source(ISSUE_RETRIES_KEY, INJECTED_RETRIES);

    let merged = args
        .load_and_merge_with_matches_with_sources(&matches, source)
        .context("merge the injected source above the file")?;

    ensure!(
        merged.retries == INJECTED_RETRIES,
        "expected the injected {INJECTED_RETRIES} above the file value {FILE_RETRIES}, got {}",
        merged.retries
    );
    Ok(())
}

/// `SubcmdConfigMerge::load_and_merge_with_matches_with_sources` keeps explicit
/// CLI values above the injected environment.
///
/// Catches a forwarding that applies the environment layer last, which would
/// leave `INJECTED_RETRIES` instead of `CLI_RETRIES`.
#[rstest]
fn matches_with_sources_method_keeps_cli_values_above_the_source(
    isolated_env: Result<IsolatedEnv>,
    empty_dir: Result<ConfigDir>,
) -> Result<()> {
    let _isolated = isolated_env?;
    let _dir = empty_dir?;
    let retries = CLI_RETRIES.to_string();
    let matches = IssueArgs::command().get_matches_from(["issue", "--retries", retries.as_str()]);
    let args = IssueArgs::from_arg_matches(&matches).context("parse explicit values")?;
    let source = scan_source(ISSUE_RETRIES_KEY, INJECTED_RETRIES);

    let merged = args
        .load_and_merge_with_matches_with_sources(&matches, source)
        .context("merge explicit CLI values over the injected source")?;

    ensure!(
        merged.retries == CLI_RETRIES,
        "expected the CLI value {CLI_RETRIES}, not the injected {INJECTED_RETRIES}: {}",
        merged.retries
    );
    Ok(())
}
