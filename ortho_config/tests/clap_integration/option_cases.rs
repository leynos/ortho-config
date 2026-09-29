//! Tests exercising optional flag handling and short-flag collisions.

use super::common::{
    ConflictConfig, OptionBoolConfig, OptionConfig, OrthoConfig, ToAnyhow, with_jail,
};
use anyhow::{Result, ensure};
use rstest::rstest;

#[rstest]
#[case::present(&["prog", "--maybe", "5"], Some(5))]
#[case::absent(&["prog"], None)]
fn parses_option_field(#[case] args: &[&str], #[case] expected: Option<u32>) -> Result<()> {
    let cfg = OptionConfig::load_from_iter(args.iter().copied()).to_anyhow()?;
    ensure!(
        cfg.maybe == expected,
        "expected maybe {:?}, got {:?}",
        expected,
        cfg.maybe
    );
    Ok(())
}

/// One row of the `Option<bool>` boolean-value matrix.
///
/// `files` and `env` supply the lower-precedence rungs; `cli_args` supplies the
/// command line. The case name records which rung is expected to win.
struct OptionBoolCase {
    files: &'static [(&'static str, &'static str)],
    env: &'static [(&'static str, &'static str)],
    cli_args: &'static [&'static str],
    expected: Option<bool>,
}

/// Covers `Option<bool>` across the spellings the issue names: absent, a bare
/// flag, an explicit value, and an explicit value clearing each lower layer.
///
/// The issue requires both `bool` and `Option<bool>` to be covered for absent,
/// true, false, file or environment true, and an explicit CLI false. The
/// `bool` half lives in `parsing.rs`; this is the `Option<bool>` half, whose
/// distinguishing case is the absent flag resolving to `None` rather than
/// `false` — a field declared `Option<bool>` must not collapse "nothing
/// supplied it anywhere" into `Some(false)`.
#[rstest]
// Nothing supplied anywhere: the field stays absent rather than defaulting.
#[case::absent(OptionBoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog"],
    expected: None,
})]
// A bare flag means true; the missing value comes from clap's
// `default_missing_value`, not from a second configuration source.
#[case::bare_flag_sets_true(OptionBoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog", "--flag"],
    expected: Some(true),
})]
#[case::explicit_true(OptionBoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog", "--flag=true"],
    expected: Some(true),
})]
#[case::explicit_false(OptionBoolCase {
    files: &[],
    env: &[],
    cli_args: &["prog", "--flag=false"],
    expected: Some(false),
})]
// A file value alone must survive, still distinguishable from an explicit
// CLI false.
#[case::file_true_without_flag(OptionBoolCase {
    files: &[(".optl.toml", "flag = true")],
    env: &[],
    cli_args: &["prog"],
    expected: Some(true),
})]
// An environment value alone must survive.
#[case::env_true_without_flag(OptionBoolCase {
    files: &[],
    env: &[("OPTL_FLAG", "true")],
    cli_args: &["prog"],
    expected: Some(true),
})]
// The CLI clears a file that set true.
#[case::cli_false_clears_file_true(OptionBoolCase {
    files: &[(".optl.toml", "flag = true")],
    env: &[],
    cli_args: &["prog", "--flag=false"],
    expected: Some(false),
})]
// The CLI clears an environment value that set true.
#[case::cli_false_clears_env_true(OptionBoolCase {
    files: &[],
    env: &[("OPTL_FLAG", "true")],
    cli_args: &["prog", "--flag=false"],
    expected: Some(false),
})]
fn parses_option_bool_across_sources(#[case] case: OptionBoolCase) -> Result<()> {
    let expected = case.expected;
    with_jail(|jail| {
        for (path, contents) in case.files {
            jail.create_file(path, contents)?;
        }
        for (key, value) in case.env {
            jail.set_env(key, value);
        }
        let config = OptionBoolConfig::load_from_iter(case.cli_args.iter().copied()).to_anyhow()?;
        ensure!(
            config.flag == expected,
            "expected flag {:?}, got {:?}",
            expected,
            config.flag
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn resolves_short_flag_conflict() -> Result<()> {
    let cfg = ConflictConfig::load_from_iter(["prog", "-s", "one", "-S", "two"]).to_anyhow()?;
    ensure!(
        cfg.second.as_deref() == Some("one"),
        "expected second one, got {:?}",
        cfg.second
    );
    ensure!(
        cfg.sample.as_deref() == Some("two"),
        "expected sample two, got {:?}",
        cfg.sample
    );
    Ok(())
}
