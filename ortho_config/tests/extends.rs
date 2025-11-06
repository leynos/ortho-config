//! Tests for configuration inheritance using the `extends` key.
use anyhow::{Result, anyhow, ensure};
#[cfg(windows)]
use std::borrow::Cow;
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
        #[cfg(windows)]
        {
            use std::borrow::Cow;

            fn canonicalish(path: &std::path::Path) -> std::path::PathBuf {
                dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
            }

            fn normalise(value: Cow<'_, str>) -> String {
                value
                    .to_ascii_lowercase()
                    .replace('/', "\\")
            }

            let msg_norm = normalise(Cow::Borrowed(msg.as_str()));
            let base_norm = normalise(expected_base.to_string_lossy());
            let canonical_norm = normalise(canonicalish(&expected_base).to_string_lossy());
            ensure!(
                msg_norm.contains(&base_norm) || msg_norm.contains(&canonical_norm),
                "error missing path variants: {msg}"
            );
        }
        #[cfg(not(windows))]
        {
            let base_str = expected_base.to_string_lossy();
            ensure!(msg.contains(base_str.as_ref()), "error missing path: {msg}");
        }
        ensure!(
            msg.contains(".config.toml"),
            "error missing config reference: {msg}"
        );
        ensure!(
            msg.contains("does not exist"),
            "error missing existence message: {msg}"
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

#[rstest]
fn empty_extends_errors() -> Result<()> {
    with_jail(|j| {
        j.create_file("base.toml", "")?; // placeholder so Jail has root file
        j.create_file(".config.toml", "extends = ''")?;
        let err = match ExtendsCfg::load_from_iter(["prog"]) {
            Ok(cfg) => return Err(anyhow!("expected empty extends error, got {cfg:?}")),
            Err(err) => err,
        };
        let display = err.to_string();
        ensure!(
            display.contains("non-empty"),
            "error missing non-empty message: {display}"
        );
        Ok(())
    })?;
    Ok(())
}

#[rstest]
fn directory_extends_errors() -> Result<()> {
    with_jail(|j| {
        j.create_dir("dir")?;
        j.create_file(".config.toml", "extends = 'dir'")?;
        let err = match ExtendsCfg::load_from_iter(["prog"]) {
            Ok(cfg) => return Err(anyhow!("expected directory extends error, got {cfg:?}")),
            Err(err) => err,
        };
        let display = err.to_string();
        ensure!(
            display.contains("not a regular file"),
            "error missing directory message: {display}"
        );
        Ok(())
    })?;
    Ok(())
}
