//! Unit tests covering helper builders that feed the global configuration layer.

use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;

use anyhow::{Result, ensure};
use rstest::{fixture, rstest};
use tempfile::Builder;

use super::super::{
    FileOverrides, GlobalArgs, Overrides, build_cli_args, build_overrides, file_excited_value,
    trimmed_salutations,
};

fn expect_override_salutations<'a>(overrides: &'a Overrides<'a>) -> Result<&'a [String]> {
    overrides
        .salutations
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("expected salutation overrides"))
}

#[fixture]
fn default_globals() -> GlobalArgs {
    GlobalArgs::default()
}

#[fixture]
fn default_file_overrides() -> FileOverrides {
    FileOverrides::default()
}

#[fixture]
fn excited_file_overrides() -> FileOverrides {
    FileOverrides {
        is_excited: Some(true),
        ..FileOverrides::default()
    }
}

#[rstest]
fn build_overrides_prioritises_cli_flags(mut default_globals: GlobalArgs) -> Result<()> {
    default_globals.is_excited = true;
    default_globals.salutations = vec![" Hello ".to_owned()];
    let overrides = build_overrides(
        &default_globals,
        trimmed_salutations(&default_globals),
        None,
        None,
    );
    ensure!(overrides.is_excited == Some(true), "cli flag should win");
    ensure!(
        expect_override_salutations(&overrides)? == ["Hello"],
        "salutations should be trimmed",
    );
    Ok(())
}

#[rstest]
fn build_overrides_uses_file_defaults_when_cli_quiet(
    default_globals: GlobalArgs,
    excited_file_overrides: FileOverrides,
) -> Result<()> {
    let overrides = build_overrides(&default_globals, None, Some(&excited_file_overrides), None);
    ensure!(
        overrides.is_excited == Some(true),
        "file defaults should populate excitement",
    );
    Ok(())
}

#[test]
fn build_cli_args_includes_override_path() {
    let args = build_cli_args(Some(Path::new("custom.toml")));
    assert!(!args.is_empty(), "expected binary name");
    let tail: Vec<_> = args.iter().skip(1).collect();
    match tail.as_slice() {
        [flag, path] => {
            assert_eq!(flag.as_os_str(), OsStr::new("--config"));
            assert_eq!(path.as_os_str(), OsStr::new("custom.toml"));
        }
        other => panic!("expected config flag and path, got {other:?}"),
    }
}

#[test]
fn build_cli_args_returns_binary_when_no_override() {
    let args = build_cli_args(None);
    assert_eq!(args.len(), 1, "expected only binary name");
}

#[rstest]
fn trimmed_salutations_returns_none_when_empty(default_globals: GlobalArgs) {
    assert!(trimmed_salutations(&default_globals).is_none());
}

#[rstest]
fn trimmed_salutations_trims_entries(mut default_globals: GlobalArgs) -> Result<()> {
    default_globals.salutations = vec!["  Hey".to_owned(), "there  ".to_owned()];
    let trimmed =
        trimmed_salutations(&default_globals).ok_or_else(|| anyhow::anyhow!("missing values"))?;
    ensure!(trimmed == ["Hey", "there"], "expected trimmed values");
    Ok(())
}

#[rstest]
fn file_excited_value_prefers_override_config() -> Result<()> {
    let mut file = Builder::new().suffix(".toml").tempfile()?;
    writeln!(file, "is_excited = true")?;
    let value = file_excited_value(None, Some(file.path()));
    ensure!(value == Some(true), "expected override config excitement");
    Ok(())
}

#[rstest]
fn file_excited_value_falls_back_to_discovered_defaults(
    excited_file_overrides: FileOverrides,
) -> Result<()> {
    let mut file = Builder::new().suffix(".toml").tempfile()?;
    writeln!(file, "is_excited = \"nope\"")?;
    let value = file_excited_value(Some(&excited_file_overrides), Some(file.path()));
    ensure!(
        value == Some(true),
        "fallback should use discovered defaults"
    );
    Ok(())
}

#[rstest]
fn file_excited_value_returns_discovered_when_no_override_path(
    mut default_file_overrides: FileOverrides,
) -> Result<()> {
    default_file_overrides.is_excited = Some(false);
    let value = file_excited_value(Some(&default_file_overrides), None);
    ensure!(value == Some(false), "expected discovered file excitement");
    Ok(())
}
