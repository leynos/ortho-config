#![expect(
    clippy::expect_used,
    reason = "tests panic when discovery helpers fail to create fixtures"
)]

use super::*;
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
fn env_override_precedes_other_candidates() {
    let (discovery, path, _guards, _env) = setup_env_override_discovery();
    let candidates = discovery.candidates();
    assert_eq!(candidates.first(), Some(&path));
}

#[rstest]
fn xdg_candidates_follow_explicit_paths() {
    let _guards = clear_common_env();
    let dir = TempDir::new().expect("xdg");
    let xdg_path = dir.path().join("hello_world");
    std::fs::create_dir_all(&xdg_path).expect("create xdg dir");
    let _home = test_env::set_var("XDG_CONFIG_HOME", dir.path());

    let discovery = ConfigDiscovery::builder("hello_world").build();
    let candidates = discovery.candidates();
    let expected_first = xdg_path.join("config.toml");
    let expected_second = dir.path().join(".hello_world.toml");
    assert_eq!(candidates.first(), Some(&expected_first));
    assert_eq!(candidates.get(1), Some(&expected_second));
}

#[cfg(any(unix, target_os = "redox"))]
#[rstest]
fn xdg_dirs_empty_falls_back_to_default() {
    let _guards = clear_common_env();
    let _dirs = test_env::set_var("XDG_CONFIG_DIRS", "");

    let discovery = ConfigDiscovery::builder("hello_world").build();
    let candidates = discovery.candidates();

    let default_base = PathBuf::from("/etc/xdg");
    let nested = default_base.join("hello_world").join("config.toml");
    let dotfile = default_base.join(".hello_world.toml");

    assert!(
        candidates.contains(&nested),
        "expected fallback nested candidate present"
    );
    assert!(
        candidates.contains(&dotfile),
        "expected fallback dotfile candidate present"
    );
}

#[cfg(any(unix, target_os = "redox"))]
#[rstest]
fn xdg_dirs_with_values_excludes_default() {
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

    assert!(
        candidates.contains(&provided_nested),
        "expected provided directory candidate present"
    );
    assert!(
        !candidates.contains(&default_nested),
        "unexpected fallback nested candidate present"
    );
    assert!(
        !candidates.contains(&default_dotfile),
        "unexpected fallback dotfile candidate present"
    );
}

#[rstest]
fn utf8_candidates_prioritise_env_paths() {
    let (discovery, path, _guards, _env) = setup_env_override_discovery();
    let mut candidates = discovery.utf8_candidates();
    assert_eq!(
        candidates.remove(0),
        Utf8PathBuf::from_path_buf(path).expect("utf8 explicit path")
    );
}

#[rstest]
fn project_roots_append_last() {
    let _guards = clear_common_env();
    let discovery = ConfigDiscovery::builder("hello_world")
        .clear_project_roots()
        .add_project_root("proj")
        .build();
    let candidates = discovery.candidates();
    assert_eq!(
        candidates.last(),
        Some(&PathBuf::from("proj/.hello_world.toml"))
    );
}

#[rstest]
fn project_roots_replaces_existing_entries() {
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
    assert!(
        candidates.len() >= expected.len(),
        "expected at least {} candidates, found {}",
        expected.len(),
        candidates.len()
    );
    assert!(candidates.ends_with(&expected));
    assert!(
        !candidates.contains(&PathBuf::from("legacy/.hello_world.toml")),
        "expected legacy project root to be cleared"
    );
}

#[derive(Debug, Deserialize)]
struct SampleConfig {
    value: bool,
}

#[rstest]
fn load_first_reads_first_existing_file() {
    let _guards = clear_common_env();
    let dir = TempDir::new().expect("config dir");
    let file_dir = dir.path().join("hello_world");
    std::fs::create_dir_all(&file_dir).expect("create hello_world dir");
    let file = file_dir.join("config.toml");
    std::fs::write(&file, "value = true").expect("write config");
    let _xdg = test_env::set_var("XDG_CONFIG_HOME", dir.path());

    let discovery = ConfigDiscovery::builder("hello_world").build();
    let figment = discovery
        .load_first()
        .expect("load figment")
        .expect("figment present");
    let config: SampleConfig = figment.extract().expect("extract sample config");
    assert!(config.value);
}

#[rstest]
fn load_first_skips_invalid_candidates() {
    let _guards = clear_common_env();
    let dir = TempDir::new().expect("config dir");
    let invalid = dir.path().join("broken.toml");
    let valid = dir.path().join("valid.toml");
    std::fs::write(&invalid, "value = ???").expect("write invalid config");
    std::fs::write(&valid, "value = false").expect("write valid config");
    let _env = test_env::set_var("HELLO_WORLD_CONFIG_PATH", &invalid);

    let discovery = ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .add_explicit_path(valid.clone())
        .build();

    let figment = discovery
        .load_first()
        .expect("load figment")
        .expect("figment present");
    let config: SampleConfig = figment.extract().expect("extract sample config");
    assert!(!config.value);
    assert!(
        std::fs::metadata(&invalid).is_ok(),
        "expected invalid file retained"
    );
}

#[rstest]
fn load_first_with_errors_reports_preceding_failures() {
    let _guards = clear_common_env();
    let dir = TempDir::new().expect("config dir");
    let missing = dir.path().join("absent.toml");
    let valid = dir.path().join("valid.toml");
    std::fs::write(&valid, "value = true").expect("write valid config");

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .add_explicit_path(valid.clone())
        .build();

    let (loaded_fig, errors) = discovery.load_first_with_errors();

    assert!(
        loaded_fig.is_some(),
        "expected successful load from valid fallback"
    );
    assert!(
        errors.iter().any(|err| match err.as_ref() {
            OrthoError::File { path, .. } => path == &missing,
            _ => false,
        }),
        "expected discovery error collection to capture missing required candidate",
    );
}

#[rstest]
fn partitioned_errors_surface_required_failures() {
    let _guards = clear_common_env();
    let dir = TempDir::new().expect("config dir");
    let missing = dir.path().join("absent.toml");
    let valid = dir.path().join("valid.toml");
    std::fs::write(&valid, "value = true").expect("write valid config");

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .add_explicit_path(valid.clone())
        .build();

    let outcome = discovery.load_first_partitioned();

    assert!(outcome.figment.is_some(), "expected fallback figment");
    assert!(
        outcome
            .required_errors
            .iter()
            .any(|err| match err.as_ref() {
                OrthoError::File { path, .. } => path == &missing,
                _ => false,
            }),
        "expected missing required candidate to be retained",
    );
    assert!(
        outcome.optional_errors.is_empty(),
        "expected optional errors to remain empty when only required path fails",
    );
}

#[rstest]
fn required_paths_emit_missing_errors() {
    let _guards = clear_common_env();
    let dir = TempDir::new().expect("config dir");
    let missing = dir.path().join("absent.toml");

    let discovery = ConfigDiscovery::builder("hello_world")
        .add_required_path(&missing)
        .build();
    let (_, errors) = discovery.load_first_with_errors();

    assert!(
        errors.iter().any(|err| match err.as_ref() {
            OrthoError::File { path, .. } => path == &missing,
            _ => false,
        }),
        "expected missing required path error"
    );
}

#[cfg(windows)]
#[rstest]
fn windows_candidates_are_case_insensitive() {
    use std::ffi::OsString;

    let _guards = clear_common_env();
    let mut builder = ConfigDiscovery::builder("hello_world");
    builder = builder.add_explicit_path(PathBuf::from("C:/Config/FILE.TOML"));
    builder = builder.add_explicit_path(PathBuf::from("c:/config/file.toml"));
    let discovery = builder.build();
    let candidates = discovery.candidates();
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].as_os_str(),
        OsString::from("C:/Config/FILE.TOML")
    );
}
