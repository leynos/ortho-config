//! Platform-specific discovery assertions.

#[cfg(windows)]
use std::path::Path;

#[cfg(windows)]
use anyhow::{Result, ensure};
#[cfg(windows)]
use rstest::rstest;
#[cfg(windows)]
use test_helpers::env::EnvScope;

#[cfg(windows)]
use super::super::*;
#[cfg(windows)]
use super::fixtures::env_guards;

#[cfg(windows)]
#[rstest]
fn windows_candidates_are_case_insensitive(env_guards: EnvScope) -> Result<()> {
    use std::ffi::OsStr;
    use std::path::PathBuf;

    let _guards = env_guards;
    let discovery = ConfigDiscovery::builder("hello_world")
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
