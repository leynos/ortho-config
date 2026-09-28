//! Fallible XDG metadata lookup tests for subcommand paths.

#[cfg(any(unix, target_os = "redox"))]
use super::{MapEnv, Prefix, TempDir, candidate_paths_at};
#[cfg(any(unix, target_os = "redox"))]
use anyhow::{Context as _, Result, ensure};
#[cfg(any(unix, target_os = "redox"))]
use std::fs;

/// Confirms non-`NotFound` XDG metadata failures are returned with their path.
#[cfg(any(unix, target_os = "redox"))]
#[test]
fn xdg_candidate_metadata_errors_are_returned() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let not_a_directory = root.path().join("not-a-directory");
    fs::write(&not_a_directory, "").context("write non-directory XDG root")?;
    let candidate = not_a_directory.join("app/config.toml");
    let expected = fs::metadata(&candidate).expect_err("file parent must reject child path");
    let source = MapEnv::new().with_var("XDG_CONFIG_HOME", &not_a_directory);

    let error = candidate_paths_at(&Prefix::new("app"), root.path(), &source)
        .expect_err("metadata failure must not be treated as absence");
    match error.as_ref() {
        crate::OrthoError::File {
            path,
            source: error_source,
        } => {
            ensure!(path == &candidate, "reported path differs: {path:?}");
            let actual = error_source
                .downcast_ref::<std::io::Error>()
                .ok_or_else(|| anyhow::anyhow!("metadata error source was not an I/O error"))?;
            ensure!(
                actual.kind() == expected.kind(),
                "metadata error kind differs: {:?} != {:?}",
                actual.kind(),
                expected.kind()
            );
        }
        other => anyhow::bail!("expected a file error, got {other:?}"),
    }
    Ok(())
}
