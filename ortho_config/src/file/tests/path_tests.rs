//! Tests covering base path resolution and graph merging.

use super::super::path::resolve_base_path;
use super::to_anyhow;
use crate::result_ext::ResultIntoFigment;
use anyhow::{Result, anyhow, ensure};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use rstest::rstest;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[rstest]
#[case::relative(false)]
#[case::absolute(true)]
fn resolve_base_path_resolves(#[case] is_abs: bool) -> Result<()> {
    let temp = TempDir::new()?;
    let dir = Dir::open_ambient_dir(temp.path(), ambient_authority())?;
    dir.write("base.toml", b"")?;
    let root = to_anyhow(super::super::canonicalise(temp.path()))?;
    let current = root.join("config.toml");
    let base_path = if is_abs {
        root.join("base.toml")
    } else {
        PathBuf::from("base.toml")
    };
    let resolved = resolve_base_path(&current, base_path).to_figment()?;
    ensure!(
        resolved == root.join("base.toml"),
        "unexpected resolved path {resolved:?}"
    );
    Ok(())
}

#[test]
fn resolve_base_path_errors_when_no_parent() -> Result<()> {
    let err = match resolve_base_path(Path::new(""), PathBuf::from("base.toml")) {
        Ok(path) => {
            return Err(anyhow!(
                "expected resolve_base_path to fail for path without parent, got {path:?}"
            ));
        }
        Err(err) => err,
    };
    ensure!(
        err.to_string()
            .contains("Cannot determine parent directory"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[rstest]
#[case::relative(false)]
#[case::absolute(true)]
fn resolve_base_path_reports_missing_file(#[case] is_abs: bool) -> Result<()> {
    let temp = TempDir::new()?;
    let root = to_anyhow(super::super::canonicalise(temp.path()))?;
    let current = root.join("config.toml");
    let expected_base = root.join("missing.toml");
    let base = if is_abs {
        expected_base.clone()
    } else {
        PathBuf::from("missing.toml")
    };
    let err = match resolve_base_path(&current, base) {
        Ok(path) => {
            return Err(anyhow!(
                "expected resolve_base_path to fail for missing file, got {path:?}"
            ));
        }
        Err(err) => err,
    };
    let msg = err.to_string();
    ensure!(
        msg.contains(expected_base.to_string_lossy().as_ref()),
        "error message {msg} does not mention missing base {expected_base:?}"
    );
    ensure!(
        msg.contains(current.to_string_lossy().as_ref()),
        "error message {msg} does not mention current path"
    );
    ensure!(msg.contains("does not exist"), "message: {msg}");
    match err.as_ref() {
        crate::OrthoError::File { source, .. } => {
            let io_err = source
                .downcast_ref::<std::io::Error>()
                .ok_or_else(|| anyhow!("expected std::io::Error source"))?;
            let actual = io_err.kind();
            ensure!(
                actual == std::io::ErrorKind::NotFound,
                "unexpected IO error kind: {actual:?}"
            );
        }
        other => {
            return Err(anyhow!("expected File error, received {other:?}"));
        }
    }
    Ok(())
}
