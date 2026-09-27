//! Platform-specific discovery assertions.

#[cfg(windows)]
use std::path::Path;
#[cfg(windows)]
use std::sync::Arc;

#[cfg(windows)]
use super::super::*;
#[cfg(windows)]
use crate::MapEnv;
#[cfg(windows)]
use anyhow::{Result, ensure};
#[cfg(windows)]
use rstest::rstest;

#[cfg(windows)]
#[rstest]
fn windows_candidates_are_case_insensitive() -> Result<()> {
    use std::ffi::OsStr;
    use std::path::PathBuf;

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new()))
        .add_explicit_path(PathBuf::from("C:/Config/FILE.TOML"))
        .add_explicit_path(PathBuf::from("c:/config/file.toml"))
        .build();
    let candidates = discovery.candidates();
    let canonical = ConfigDiscovery::normalized_key(Path::new("C:/Config/FILE.TOML"));
    let duplicates = candidates
        .iter()
        .filter(|candidate| ConfigDiscovery::normalized_key(candidate.as_path()) == canonical)
        .count();
    ensure!(
        duplicates == 1,
        "expected canonical key {canonical:?} to appear once; observed {duplicates} entries: {candidates:?}",
    );
    ensure!(
        candidates.first().map(|c| c.as_os_str()) == Some(OsStr::new("C:/Config/FILE.TOML")),
        "expected original casing preserved"
    );
    Ok(())
}
