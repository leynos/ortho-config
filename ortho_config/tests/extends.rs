//! Tests for configuration inheritance using the `extends` key.
use anyhow::{Result, anyhow, ensure};
use ortho_config::{OrthoConfig, OrthoError};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[path = "test_utils.rs"]
mod test_utils;
use test_utils::with_jail;

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct ExtendsCfg {
    #[serde(skip_serializing_if = "Option::is_none")]
    foo: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct MultiLevelCfg {
    app_name: String,
    retries: u8,
    enabled: bool,
    tags: Vec<String>,
    #[ortho_config(skip_cli)]
    nested: MultiLevelNested,
    parent_only: Option<String>,
    child_only: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct MultiLevelNested {
    region: String,
    threshold: u8,
    mode: Option<String>,
}

const GRANDPARENT_TOML: &str = concat!(
    "app_name = \"base\"\n",
    "retries = 1\n",
    "enabled = true\n",
    "tags = [\"base\"]\n",
    "[nested]\n",
    "region = \"base\"\n",
    "threshold = 1\n",
);

const PARENT_TOML: &str = concat!(
    "extends = \"grandparent.toml\"\n",
    "retries = 2\n",
    "tags = [\"parent\"]\n",
    "parent_only = \"parent\"\n",
    "[nested]\n",
    "threshold = 2\n",
    "mode = \"parent\"\n",
);

const CHILD_TOML: &str = concat!(
    "extends = \"parent.toml\"\n",
    "enabled = false\n",
    "tags = [\"child\"]\n",
    "child_only = \"child\"\n",
    "[nested]\n",
    "region = \"child\"\n",
);

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
fn multi_level_inheritance_merges_in_order() -> Result<()> {
    with_jail(|j| {
        setup_multi_level_test_files(j)?;
        let cfg = MultiLevelCfg::load_from_iter(["prog"]).map_err(|err| anyhow!(err))?;
        verify_multi_level_config(&cfg)?;
        Ok(())
    })?;
    Ok(())
}

fn setup_multi_level_test_files(j: &mut figment::Jail) -> Result<()> {
    j.create_file("grandparent.toml", GRANDPARENT_TOML)?;
    j.create_file("parent.toml", PARENT_TOML)?;
    j.create_file(".config.toml", CHILD_TOML)?;
    Ok(())
}

fn ensure_eq<T>(actual: &T, expected: &T, label: &str) -> Result<()>
where
    T: PartialEq + std::fmt::Debug,
{
    ensure!(
        actual == expected,
        "unexpected {label} {actual:?}; expected {expected:?}"
    );
    Ok(())
}

fn verify_multi_level_config(cfg: &MultiLevelCfg) -> Result<()> {
    ensure_eq(&cfg.app_name.as_str(), &"base", "app_name")?;
    ensure_eq(&cfg.retries, &2, "retries")?;
    ensure_eq(&cfg.enabled, &false, "enabled")?;
    let expected_tags = vec![String::from("child")];
    ensure_eq(&cfg.tags, &expected_tags, "tags")?;
    ensure_eq(&cfg.nested.region.as_str(), &"child", "nested.region")?;
    ensure_eq(&cfg.nested.threshold, &2, "nested.threshold")?;
    ensure_eq(&cfg.nested.mode.as_deref(), &Some("parent"), "nested.mode")?;
    ensure_eq(&cfg.parent_only.as_deref(), &Some("parent"), "parent_only")?;
    ensure_eq(&cfg.child_only.as_deref(), &Some("child"), "child_only")?;
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
    EmptyFile(PathBuf),
    Directory(PathBuf),
}

#[rstest]
#[case::empty_extends(
    SetupType::EmptyFile(PathBuf::from("base.toml")),
    "",
    "non-empty",
    "empty extends"
)]
#[case::directory_extends(
    SetupType::Directory(PathBuf::from("dir")),
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
        |j| match &setup {
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
