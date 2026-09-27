//! Unit tests for XDG candidate probing failures in subcommand path resolution.
//!
//! Only a missing path counts as an absent candidate; any other probe failure
//! must reach the caller instead of silently dropping a configuration location.
//!
//! The module is Unix-only: it is compiled solely under
//! `cfg(any(unix, target_os = "redox"))`, matching the platform that provides
//! the XDG probing rules it exercises.

use super::*;
use crate::{MapEnv, OrthoError};
use anyhow::{Context, Result, ensure};
use std::path::PathBuf;
use tempfile::TempDir;

/// A candidate that does not exist is reported as absent rather than an error.
#[test]
fn missing_xdg_candidate_is_absent() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let exists = xdg_candidate_exists(&root.path().join("missing/app/config.toml"))?;
    ensure!(!exists, "a missing candidate must be absent");
    Ok(())
}

/// A missing candidate is skipped so the next base still wins.
#[test]
fn first_existing_xdg_candidate_skips_missing_bases() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let present = root.path().join("second/config.toml");
    std::fs::create_dir_all(present.parent().context("find config parent")?)
        .context("create second base")?;
    std::fs::write(&present, "").context("write present candidate")?;
    let bases = vec![root.path().join("first"), root.path().join("second")];
    let selected = first_existing_xdg_candidate(&bases, "config.toml")?;
    ensure!(
        selected.as_ref() == Some(&present),
        "expected {present:?}, got {selected:?}"
    );
    Ok(())
}

/// A candidate under an unsearchable directory fails loading instead of being
/// skipped.
///
/// Privileged users bypass permission bits, so the probe cannot fail for them;
/// the test detects that and returns early rather than asserting a false
/// negative.
#[cfg(unix)]
#[test]
fn unsearchable_xdg_directory_propagates_probe_error() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let root = TempDir::new().context("create root")?;
    let locked = root.path().join("locked");
    std::fs::create_dir_all(locked.join("app")).context("create locked XDG tree")?;
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000))
        .context("lock XDG directory")?;
    let is_privileged = std::fs::metadata(locked.join("app")).is_ok();
    let env = MapEnv::new().with_var("XDG_CONFIG_HOME", &locked);
    let result =
        (!is_privileged).then(|| candidate_paths_at(&Prefix::new("app"), root.path(), &env));
    // Restore access before asserting so the temporary tree can be removed.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700))
        .context("unlock XDG directory")?;
    let Some(outcome) = result else {
        return Ok(());
    };

    let error = outcome
        .err()
        .context("a permission-denied probe must propagate")?;
    let OrthoError::File { path, source } = error.as_ref() else {
        anyhow::bail!("expected a file error, got {error:?}");
    };
    let expected: PathBuf = locked.join("app/config.toml");
    ensure!(
        path == &expected,
        "error must name the probed candidate, got {path:?}"
    );
    let io_error = source
        .downcast_ref::<std::io::Error>()
        .context("error source must be the probe's I/O error")?;
    ensure!(
        io_error.kind() == std::io::ErrorKind::PermissionDenied,
        "unexpected probe error kind: {io_error:?}"
    );
    Ok(())
}
