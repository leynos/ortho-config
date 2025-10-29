//! Unit tests covering helper builders that feed the global configuration layer.

use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;

use anyhow::{Result, ensure};
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

#[test]
fn build_overrides_prioritises_cli_flags() -> Result<()> {
    let globals = GlobalArgs {
        is_excited: true,
        salutations: vec![" Hello ".to_owned()],
        ..GlobalArgs::default()
    };
    let overrides = build_overrides(&globals, trimmed_salutations(&globals), None, None);
    ensure!(overrides.is_excited == Some(true), "cli flag should win");
    ensure!(
        expect_override_salutations(&overrides)? == ["Hello"],
        "salutations should be trimmed",
    );
    Ok(())
}

#[test]
fn build_overrides_uses_file_defaults_when_cli_quiet() -> Result<()> {
    let globals = GlobalArgs::default();
    let file = FileOverrides {
        is_excited: Some(true),
        ..FileOverrides::default()
    };
    let overrides = build_overrides(&globals, None, Some(&file), None);
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

#[test]
fn trimmed_salutations_returns_none_when_empty() {
    let globals = GlobalArgs::default();
    assert!(trimmed_salutations(&globals).is_none());
}

#[test]
fn trimmed_salutations_trims_entries() -> Result<()> {
    let globals = GlobalArgs {
        salutations: vec!["  Hey".to_owned(), "there  ".to_owned()],
        ..GlobalArgs::default()
    };
    let trimmed = trimmed_salutations(&globals).ok_or_else(|| anyhow::anyhow!("missing values"))?;
    ensure!(trimmed == ["Hey", "there"], "expected trimmed values");
    Ok(())
}

#[test]
fn file_excited_value_prefers_override_config() -> Result<()> {
    let mut file = Builder::new().suffix(".toml").tempfile()?;
    writeln!(file, "is_excited = true")?;
    let value = file_excited_value(None, Some(file.path()));
    ensure!(value == Some(true), "expected override config excitement");
    Ok(())
}

#[test]
fn file_excited_value_falls_back_to_discovered_defaults() -> Result<()> {
    let mut file = Builder::new().suffix(".toml").tempfile()?;
    writeln!(file, "is_excited = \"nope\"")?;
    let overrides = FileOverrides {
        is_excited: Some(true),
        ..FileOverrides::default()
    };
    let value = file_excited_value(Some(&overrides), Some(file.path()));
    ensure!(
        value == Some(true),
        "fallback should use discovered defaults"
    );
    Ok(())
}

#[test]
fn file_excited_value_returns_discovered_when_no_override_path() -> Result<()> {
    let overrides = FileOverrides {
        is_excited: Some(false),
        ..FileOverrides::default()
    };
    let value = file_excited_value(Some(&overrides), None);
    ensure!(value == Some(false), "expected discovered file excitement");
    Ok(())
}
