//! Candidate-ordering tests for discovery.

use std::path::PathBuf;

use anyhow::{Result, anyhow, ensure};
use camino::Utf8PathBuf;
use rstest::rstest;
use test_helpers::env::{self as test_env, EnvScope};

use super::super::*;
use super::fixtures::{config_temp_dir, env_guards, env_override_discovery};

#[rstest]
fn env_override_precedes_other_candidates(
    env_override_discovery: (ConfigDiscovery, PathBuf, EnvScope, test_env::EnvVarGuard),
) -> Result<()> {
    let (discovery, path, _scope, _env) = env_override_discovery;
    let candidates = discovery.candidates();
    ensure!(
        candidates.first() == Some(&path),
        "expected explicit env override candidate to appear first"
    );
    Ok(())
}

#[rstest]
fn xdg_candidates_follow_explicit_paths(
    env_guards: EnvScope,
    config_temp_dir: Result<tempfile::TempDir>,
) -> Result<()> {
    let _guards = env_guards;
    let temp_dir = config_temp_dir?;
    let xdg_path = temp_dir.path().join("hello_world");
    std::fs::create_dir_all(&xdg_path).map_err(anyhow::Error::new)?;
    let _home = test_env::set_var("XDG_CONFIG_HOME", temp_dir.path());

    let discovery = ConfigDiscovery::builder("hello_world").build();
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
fn xdg_dirs_empty_falls_back_to_default(env_guards: EnvScope) -> Result<()> {
    let _guards = env_guards;
    let _dirs = test_env::set_var("XDG_CONFIG_DIRS", "");

    let discovery = ConfigDiscovery::builder("hello_world").build();
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
fn xdg_dirs_with_values_excludes_default(env_guards: EnvScope) -> Result<()> {
    let _guards = env_guards;
    let _dirs = test_env::set_var("XDG_CONFIG_DIRS", "/opt/example:/etc/custom");

    let discovery = ConfigDiscovery::builder("hello_world").build();
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
    env_override_discovery: (ConfigDiscovery, PathBuf, EnvScope, test_env::EnvVarGuard),
) -> Result<()> {
    let (discovery, path, _scope, _env) = env_override_discovery;
    let candidates = discovery.utf8_candidates();
    let first = candidates
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("expected at least one UTF-8 candidate"))?;
    let expected =
        Utf8PathBuf::from_path_buf(path).map_err(|_| anyhow!("explicit path not valid UTF-8"))?;
    ensure!(
        first == expected,
        "unexpected first candidate {first:?}, expected {expected:?}"
    );
    Ok(())
}

#[rstest]
fn project_roots_append_last(env_guards: EnvScope) -> Result<()> {
    let _guards = env_guards;
    let discovery = ConfigDiscovery::builder("hello_world")
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
fn project_roots_replaces_existing_entries(env_guards: EnvScope) -> Result<()> {
    let _guards = env_guards;
    let discovery = ConfigDiscovery::builder("hello_world")
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
