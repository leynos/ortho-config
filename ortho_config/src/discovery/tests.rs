use super::*;
use anyhow::{Context, Result, anyhow, ensure};
use rstest::rstest;
use serde::Deserialize;
use tempfile::TempDir;
use test_helpers::env as test_env;

fn clear_common_env() -> Vec<test_env::EnvVarGuard> {
    let mut guards = Vec::new();
    for key in [
        "HELLO_WORLD_CONFIG_PATH",
        "XDG_CONFIG_HOME",
        "XDG_CONFIG_DIRS",
        "APPDATA",
        "LOCALAPPDATA",
        "HOME",
        "USERPROFILE",
    ] {
        guards.push(test_env::remove_var(key));
    }
    guards
}

fn setup_env_override_discovery() -> (
    ConfigDiscovery,
    PathBuf,
    Vec<test_env::EnvVarGuard>,
    test_env::EnvVarGuard,
) {
    let guards = clear_common_env();
    let path = std::env::temp_dir().join("explicit.toml");
    let env_guard = test_env::set_var("HELLO_WORLD_CONFIG_PATH", &path);
    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .build();

    (discovery, path, guards, env_guard)
}

#[rstest]
fn env_override_precedes_other_candidates() -> Result<()> {
    let (discovery, path, _guards, _env) = setup_env_override_discovery();
    let candidates = discovery.candidates();
    ensure!(
        candidates.first() == Some(&path),
        "expected explicit env override candidate to appear first"
    );
    Ok(())
}

#[rstest]
fn xdg_candidates_follow_explicit_paths() -> Result<()> {
    let _guards = clear_common_env();
    let dir = TempDir::new().context("create XDG temp directory")?;
    let xdg_path = dir.path().join("hello_world");
    std::fs::create_dir_all(&xdg_path).context("create hello_world directory under XDG home")?;
    let _home = test_env::set_var("XDG_CONFIG_HOME", dir.path());

    let discovery = ConfigDiscovery::builder("hello_world").build();
    let candidates = discovery.candidates();
    let expected_first = xdg_path.join("config.toml");
    let expected_second = dir.path().join(".hello_world.toml");
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
    let _guards = clear_common_env();
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
fn xdg_dirs_with_values_excludes_default() -> Result<()> {
    let _guards = clear_common_env();
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
fn utf8_candidates_prioritise_env_paths() -> Result<()> {
    let (discovery, path, _guards, _env) = setup_env_override_discovery();
    let candidates = discovery.utf8_candidates();
    let first = candidates
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("expected at least one UTF-8 candidate"))?;
    let expected =
        Utf8PathBuf::from_path_buf(path).map_err(|_| anyhow!("explicit path not valid UTF-8"))?;
    ensure!(
        first == expected,
        "unexpected first candidate {:?}, expected {:?}",
        first,
        expected
    );
    Ok(())
}

#[rstest]
fn project_roots_append_last() -> Result<()> {
    let _guards = clear_common_env();
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
fn project_roots_replaces_existing_entries() -> Result<()> {
    let _guards = clear_common_env();
    let discovery = ConfigDiscovery::builder("hello_world")
        .add_project_root("legacy")
        .project_roots([PathBuf::from("alpha"), PathBuf::from("beta")])
        .build();

    let candidates = discovery.candidates();
    let expected = vec![
        PathBuf::from("alpha/.hello_world.toml"),
        PathBuf::from("beta/.hello_world.toml"),
    ];
    ensure!(
        candidates.len() >= expected.len(),
        "expected at least {} candidates, found {}",
        expected.len(),
        candidates.len()
    );
    ensure!(
        candidates.ends_with(&expected),
        "expected configured project roots to appear at end; found {:?}",
        candidates
    );
    ensure!(
        !candidates.contains(&PathBuf::from("legacy/.hello_world.toml")),
        "expected legacy project root to be cleared"
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
struct SampleConfig {
    value: bool,
}

#[rstest]
fn load_first_reads_first_existing_file() -> Result<()> {
    let _guards = clear_common_env();
    let dir = TempDir::new().context("create config directory")?;
    let file_dir = dir.path().join("hello_world");
    std::fs::create_dir_all(&file_dir).context("create hello_world directory")?;
    let file = file_dir.join("config.toml");
    std::fs::write(&file, "value = true").context("write config file")?;
    let _xdg = test_env::set_var("XDG_CONFIG_HOME", dir.path());

    let discovery = ConfigDiscovery::builder("hello_world").build();
    let figment = match discovery.load_first() {
        Ok(Some(figment)) => figment,
        Ok(None) => return Err(anyhow!("expected configuration candidate to load")),
        Err(err) => return Err(anyhow!("discovery failed to load config: {err}")),
    };
    let config: SampleConfig = figment
        .extract()
        .context("extract sample config from figment")?;
    ensure!(config.value, "expected loaded config to set value=true");
    Ok(())
}

#[rstest]
fn load_first_skips_invalid_candidates() -> Result<()> {
    let _guards = clear_common_env();
    let dir = TempDir::new().context("create config directory")?;
    let invalid = dir.path().join("broken.toml");
    let valid = dir.path().join("valid.toml");
    std::fs::write(&invalid, "value = ???").context("write invalid config")?;
    std::fs::write(&valid, "value = false").context("write valid config")?;
    let _env = test_env::set_var("HELLO_WORLD_CONFIG_PATH", &invalid);

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .add_explicit_path(valid.clone())
        .build();

    let figment = match discovery.load_first() {
        Ok(Some(figment)) => figment,
        Ok(None) => return Err(anyhow!("expected fallback configuration to load")),
        Err(err) => return Err(anyhow!("discovery failed to load configuration: {err}")),
    };
    let config: SampleConfig = figment
        .extract()
        .context("extract sample config from figment")?;
    ensure!(
        !config.value,
        "expected valid candidate to override invalid env file"
    );
    ensure!(
        std::fs::metadata(&invalid).is_ok(),
        "expected invalid file retained for later diagnostics"
    );
    Ok(())
}

#[rstest]
fn load_first_with_errors_reports_preceding_failures() -> Result<()> {
    let _guards = clear_common_env();
    let dir = TempDir::new().context("create config directory")?;
    let missing = dir.path().join("absent.toml");
    let valid = dir.path().join("valid.toml");
    std::fs::write(&valid, "value = true").context("write valid config")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .add_explicit_path(valid.clone())
        .build();

    let (loaded_fig, errors) = discovery.load_first_with_errors();

    ensure!(
        loaded_fig.is_some(),
        "expected successful load from valid fallback"
    );
    ensure!(
        errors.iter().any(|err| match err.as_ref() {
            OrthoError::File { path, .. } => path == &missing,
            _ => false,
        }),
        "expected discovery error collection to capture missing required candidate",
    );
    Ok(())
}

#[rstest]
fn partitioned_errors_surface_required_failures() -> Result<()> {
    let _guards = clear_common_env();
    let dir = TempDir::new().context("create config directory")?;
    let missing = dir.path().join("absent.toml");
    let valid = dir.path().join("valid.toml");
    std::fs::write(&valid, "value = true").context("write valid config")?;

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .add_explicit_path(valid.clone())
        .build();

    let outcome = discovery.load_first_partitioned();

    ensure!(outcome.figment.is_some(), "expected fallback figment");
    ensure!(
        outcome
            .required_errors
            .iter()
            .any(|err| match err.as_ref() {
                OrthoError::File { path, .. } => path == &missing,
                _ => false,
            }),
        "expected missing required candidate to be retained",
    );
    ensure!(
        outcome.optional_errors.is_empty(),
        "expected optional errors to remain empty when only required path fails",
    );
    Ok(())
}

#[rstest]
fn required_paths_emit_missing_errors() -> Result<()> {
    let _guards = clear_common_env();
    let dir = TempDir::new().context("create config directory")?;
    let missing = dir.path().join("absent.toml");

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .build();
    let (_, errors) = discovery.load_first_with_errors();

    ensure!(
        errors.iter().any(|err| match err.as_ref() {
            OrthoError::File { path, .. } => path == &missing,
            _ => false,
        }),
        "expected missing required path error"
    );
    Ok(())
}

#[cfg(windows)]
#[rstest]
fn windows_candidates_are_case_insensitive() -> Result<()> {
    use std::ffi::OsString;

    let _guards = clear_common_env();
    let mut builder = ConfigDiscovery::builder("hello_world");
    builder = builder.add_explicit_path(PathBuf::from("C:/Config/FILE.TOML"));
    builder = builder.add_explicit_path(PathBuf::from("c:/config/file.toml"));
    let discovery = builder.build();
    let candidates = discovery.candidates();
    ensure!(
        candidates.len() == 1,
        "expected duplicate paths deduplicated"
    );
    ensure!(
        candidates[0].as_os_str() == OsString::from("C:/Config/FILE.TOML"),
        "expected original casing preserved"
    );
    Ok(())
}
