//! Candidate-ordering tests for discovery.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;

use super::super::*;
use super::fixtures::{config_temp_dir, env_override_discovery};
use crate::MapEnv;

#[rstest]
fn env_override_precedes_other_candidates(
    env_override_discovery: Result<(ConfigDiscovery, Utf8PathBuf)>,
) -> Result<()> {
    let (discovery, path) = env_override_discovery?;
    let candidates = discovery.candidates();
    ensure!(
        candidates.first().map(PathBuf::as_path) == Some(path.as_std_path()),
        "expected explicit env override candidate to appear first"
    );
    Ok(())
}

#[rstest]
fn xdg_candidates_follow_explicit_paths(config_temp_dir: Result<tempfile::TempDir>) -> Result<()> {
    let temp_dir = config_temp_dir?;
    let xdg_path = temp_dir.path().join("hello_world");
    std::fs::create_dir_all(&xdg_path).map_err(anyhow::Error::new)?;
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(
            MapEnv::new().with_var("XDG_CONFIG_HOME", temp_dir.path()),
        ))
        .build();
    let candidates = discovery.candidates();
    let expected_first = xdg_path.join("config.toml");
    let expected_second = temp_dir.path().join(".hello_world.toml");
    ensure!(
        candidates.first() == Some(&expected_first),
        "expected XDG config file candidate first"
    );
    ensure!(
        candidates.get(1) == Some(&expected_second),
        "expected XDG dotfile candidate second"
    );
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[rstest]
fn xdg_dirs_empty_falls_back_to_default() -> Result<()> {
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new().with_var("XDG_CONFIG_DIRS", "")))
        .build();
    let candidates = discovery.candidates();

    let default_base = PathBuf::from("/etc/xdg");
    let nested = default_base.join("hello_world").join("config.toml");
    let dotfile = default_base.join(".hello_world.toml");

    ensure!(
        candidates.contains(&nested),
        "expected fallback nested candidate present"
    );
    ensure!(
        candidates.contains(&dotfile),
        "expected fallback dotfile candidate present"
    );
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[rstest]
fn xdg_dirs_with_values_excludes_default() -> Result<()> {
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(
            MapEnv::new().with_var("XDG_CONFIG_DIRS", "/opt/example:/etc/custom"),
        ))
        .build();
    let candidates = discovery.candidates();

    let default_base = PathBuf::from("/etc/xdg");
    let default_nested = default_base.join("hello_world").join("config.toml");
    let default_dotfile = default_base.join(".hello_world.toml");
    let provided_nested = PathBuf::from("/opt/example")
        .join("hello_world")
        .join("config.toml");

    ensure!(
        candidates.contains(&provided_nested),
        "expected provided directory candidate present"
    );
    ensure!(
        !candidates.contains(&default_nested),
        "unexpected fallback nested candidate present"
    );
    ensure!(
        !candidates.contains(&default_dotfile),
        "unexpected fallback dotfile candidate present"
    );
    Ok(())
}

#[rstest]
fn utf8_candidates_prioritise_env_paths(
    env_override_discovery: Result<(ConfigDiscovery, Utf8PathBuf)>,
) -> Result<()> {
    let (discovery, path) = env_override_discovery?;
    let candidates = discovery.utf8_candidates();
    let first = candidates
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("expected at least one UTF-8 candidate"))?;
    ensure!(
        first == path,
        "unexpected first candidate {first:?}, expected {path:?}"
    );
    Ok(())
}

#[rstest]
fn project_roots_append_last() -> Result<()> {
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new()))
        .clear_project_roots()
        .add_project_root("proj")
        .build();
    let candidates = discovery.candidates();
    ensure!(
        candidates.last() == Some(&PathBuf::from("proj/.hello_world.toml")),
        "expected project root candidate appended last"
    );
    Ok(())
}

#[rstest]
fn project_roots_replaces_existing_entries() -> Result<()> {
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_source(Arc::new(MapEnv::new()))
        .add_project_root("legacy")
        .project_roots([PathBuf::from("alpha"), PathBuf::from("beta")])
        .build();

    let candidates = discovery.candidates();
    let expected = vec![
        PathBuf::from("alpha/.hello_world.toml"),
        PathBuf::from("beta/.hello_world.toml"),
    ];
    let actual_len = candidates.len();
    let expected_len = expected.len();
    ensure!(
        actual_len >= expected_len,
        "expected at least {expected_len} candidates, found {actual_len}"
    );
    ensure!(
        candidates.ends_with(&expected),
        "expected configured project roots to appear at end; found {candidates:?}"
    );
    ensure!(
        !candidates.contains(&PathBuf::from("legacy/.hello_world.toml")),
        "expected legacy project root to be cleared"
    );
    Ok(())
}

/// Two candidates differing only in a non-UTF-8 byte both survive dedup.
///
/// Lossy keying mapped every invalid byte to U+FFFD, so these two paths
/// produced one key and discovery silently dropped the second candidate.
#[cfg(unix)]
#[test]
fn non_utf8_candidates_stay_distinct() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;

    let first = PathBuf::from(OsString::from_vec(b"/tmp/demo-\xff.toml".to_vec()));
    let second = PathBuf::from(OsString::from_vec(b"/tmp/demo-\xfe.toml".to_vec()));

    let discovery = crate::discovery::ConfigDiscovery::builder("demo")
        .clear_project_roots()
        .env_source(std::sync::Arc::new(crate::MapEnv::new()))
        .add_explicit_path(&first)
        .add_explicit_path(&second)
        .build();

    let candidates = discovery.candidates();
    assert!(
        candidates.contains(&first) && candidates.contains(&second),
        "both non-UTF-8 candidates must survive deduplication: {candidates:?}"
    );
    let first_pos = candidates.iter().position(|path| path == &first);
    let second_pos = candidates.iter().position(|path| path == &second);
    assert!(first_pos < second_pos, "explicit order must be preserved");
}
