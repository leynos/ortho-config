//! Tests for configuration inheritance using the `extends` key.
use anyhow::{Result, anyhow, ensure};
use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::with_jail;

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ExtendsCfg {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
}

struct InheritanceCase {
    base_value: &'static str,
    config_value: &'static str,
    cli_args: &'static [&'static str],
    env_value: Option<&'static str>,
    expected: &'static str,
}

#[rstest]
#[case(
    InheritanceCase {
        base_value: "base",
        config_value: "child",
        cli_args: &[] as &[&str],
        env_value: None,
        expected: "child",
    }
)]
#[case(
    InheritanceCase {
        base_value: "base",
        config_value: "file",
        cli_args: &["--foo", "cli"],
        env_value: Some("env"),
        expected: "cli",
    }
)]
fn inheritance_precedence(#[case] case: InheritanceCase) -> Result<()> {
    with_jail(|j| {
        j.create_file("base.toml", &format!("foo = \"{}\"", case.base_value))?;
        j.create_file(
            ".config.toml",
            &format!("extends = \"base.toml\"\nfoo = \"{}\"", case.config_value),
        )?;
        if let Some(val) = case.env_value {
            j.set_env("FOO", val);
        }
        let mut args = vec!["prog"];
        args.extend_from_slice(case.cli_args);
        let cfg = ExtendsCfg::load_from_iter(args).map_err(|err| anyhow!(err))?;
        let actual = cfg.foo.as_deref();
        let expected = case.expected;
        ensure!(
            actual == Some(expected),
            "expected foo {expected}, got {actual:?}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn cyclic_inheritance_is_detected() -> Result<()> {
    with_jail(|j| {
        j.create_file("a.toml", "extends = \"b.toml\"\nfoo = \"a\"")?;
        j.create_file("b.toml", "extends = \"a.toml\"\nfoo = \"b\"")?;
        j.create_file(".config.toml", "extends = \"a.toml\"")?;
        let err = match ExtendsCfg::load_from_iter(["prog"]) {
            Ok(cfg) => return Err(anyhow!("expected cyclic extends error, got {cfg:?}")),
            Err(err) => err,
        };
        ensure!(
            matches!(&*err, OrthoError::CyclicExtends { .. }),
            "unexpected error: {err:?}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
#[cfg_attr(
    not(any(windows, target_os = "macos")),
    ignore = "case-insensitive cycle detection requires Windows or macOS"
)]
fn cyclic_inheritance_detects_case_variants() -> Result<()> {
    with_jail(|j| {
        j.create_file("Base.toml", "extends = \".CONFIG.toml\"\nfoo = \"base\"")?;
        j.create_file(".config.toml", "extends = \"base.toml\"\nfoo = \"config\"")?;
        let err = match ExtendsCfg::load_from_iter(["prog"]) {
            Ok(cfg) => return Err(anyhow!("expected cyclic extends error, got {cfg:?}")),
            Err(err) => err,
        };
        ensure!(
            matches!(&*err, OrthoError::CyclicExtends { .. }),
            "unexpected error: {err:?}"
        );
        let msg = err.to_string();
        let lower = msg.to_ascii_lowercase();
        ensure!(
            lower.contains("base.toml"),
            "error missing base reference: {msg}"
        );
        ensure!(
            lower.contains(".config.toml"),
            "error missing config reference: {msg}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
#[case::relative(false)]
#[case::absolute(true)]
fn missing_base_file_errors(#[case] is_abs: bool) -> Result<()> {
    with_jail(|j| {
        let root = std::env::current_dir().map_err(|err| anyhow!(err))?;
        let expected_base = root.join("missing.toml");
        let extends_value = if is_abs {
            expected_base.display().to_string()
        } else {
            String::from("missing.toml")
        };
        j.create_file(".config.toml", &format!("extends = {extends_value:?}"))?;
        let err = match ExtendsCfg::load_from_iter(["prog"]) {
            Ok(cfg) => return Err(anyhow!("expected missing base error, got {cfg:?}")),
            Err(err) => err,
        };
        let msg = err.to_string();
        ensure!(
            msg.contains("missing.toml"),
            "error missing filename reference: {msg}"
        );
        ensure!(
            msg.contains(".config.toml"),
            "error missing config reference: {msg}"
        );
        ensure!(
            msg.contains("does not exist"),
            "error missing existence message: {msg}"
        );
        #[cfg(windows)]
        ensure!(
            msg.contains("extended configuration file"),
            "error missing extended configuration context: {msg}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn non_string_extends_errors() -> Result<()> {
    with_jail(|j| {
        j.create_file(".config.toml", "extends = 1")?;
        let err = match ExtendsCfg::load_from_iter(["prog"]) {
            Ok(cfg) => return Err(anyhow!("expected non-string extends error, got {cfg:?}")),
            Err(err) => err,
        };
        let msg = err.to_string();
        ensure!(
            msg.contains("must be a string"),
            "error missing string message: {msg}"
        );
        ensure!(
            msg.contains(".config.toml"),
            "error missing origin mention: {msg}"
        );
        Ok(())
    })?;
    Ok(())
}

fn assert_extends_error<F>(
    setup: F,
    extends_value: &str,
    expected_msg: &str,
    error_desc: &str,
) -> Result<()>
where
    F: Fn(&figment::Jail) -> Result<()>,
{
    with_jail(|j| {
        setup(j)?;
        j.create_file(".config.toml", &format!("extends = '{extends_value}'"))?;
        let err = match ExtendsCfg::load_from_iter(["prog"]) {
            Ok(cfg) => return Err(anyhow!("expected {error_desc} error, got {cfg:?}")),
            Err(err) => err,
        };
        let display = err.to_string();
        ensure!(
            display.contains(expected_msg),
            "error missing {expected_msg:?}: {display}"
        );
        Ok(())
    })
}

enum SetupType {
    EmptyFile(&'static str),
    Directory(&'static str),
}

#[rstest]
#[case::empty_extends(SetupType::EmptyFile("base.toml"), "", "non-empty", "empty extends")]
#[case::directory_extends(
    SetupType::Directory("dir"),
    "dir",
    "not a regular file",
    "directory extends"
)]
fn extends_validation_errors(
    #[case] setup: SetupType,
    #[case] extends_value: &str,
    #[case] expected_msg: &str,
    #[case] error_desc: &str,
) -> Result<()> {
    assert_extends_error(
        |j| match setup {
            SetupType::EmptyFile(path) => {
                j.create_file(path, "")?;
                Ok(())
            }
            SetupType::Directory(path) => {
                j.create_dir(path)?;
                Ok(())
            }
        },
        extends_value,
        expected_msg,
        error_desc,
    )
}
