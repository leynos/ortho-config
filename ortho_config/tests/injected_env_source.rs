//! Discovery driven entirely by an injected environment source.
//!
//! These tests deliberately mutate nothing. They run concurrently by default —
//! no `#[serial]`, no environment lock — which is the property the injected
//! source exists to provide. Adding a process mutation to this file would
//! silently reintroduce the coupling it was written to demonstrate is gone.

use ortho_config::{ConfigDiscovery, EnvSource, MapEnv};
use rstest::rstest;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn discovery_with(env: MapEnv) -> ConfigDiscovery {
    ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .clear_project_roots()
        .add_project_root(Path::new("/workspace"))
        .env_source(Arc::new(env))
        .build()
}

#[test]
fn selector_from_injected_source_is_the_first_candidate() {
    let discovery = discovery_with(MapEnv::new().with_var("DEMO_CONFIG", "/etc/selected.toml"));
    let candidates = discovery.candidates();
    assert_eq!(
        candidates.first().map(PathBuf::as_path),
        Some(Path::new("/etc/selected.toml")),
        "the selector must win, got {candidates:?}"
    );
}

#[test]
fn empty_selector_is_ignored() {
    let discovery = discovery_with(MapEnv::new().with_var("DEMO_CONFIG", ""));
    assert!(
        !discovery
            .candidates()
            .iter()
            .any(|p| p.as_os_str().is_empty()),
        "an empty selector must not contribute a candidate"
    );
}

/// Platform base directories are taken from the injected source.
///
/// `XDG_CONFIG_HOME` and `HOME` seed different candidate generators but share
/// one assertion shape, so they are parameterized rather than duplicated.
#[rstest]
#[case::xdg_config_home("XDG_CONFIG_HOME", "/xdg", "/xdg/demo")]
#[case::home("HOME", "/home/injected", "/home/injected")]
fn base_directory_from_injected_source_is_honoured(
    #[case] key: &str,
    #[case] value: &str,
    #[case] expected_prefix: &str,
) {
    let discovery = discovery_with(MapEnv::new().with_var(key, value));
    let candidates = discovery.candidates();
    assert!(
        candidates.iter().any(|p| p.starts_with(expected_prefix)),
        "expected a candidate under {expected_prefix}, got {candidates:?}"
    );
}

/// With no home in the source, no host home may leak into the candidate list.
///
/// This is the property that makes the suite machine-independent: the platform
/// `home_dir()` fallback must be suppressed for an injected source, or the
/// candidates would differ between developer machines and CI.
#[test]
fn absent_home_does_not_fall_back_to_the_host() {
    let discovery = discovery_with(MapEnv::new());
    let host_home = dirs::home_dir();
    if let Some(host) = host_home {
        assert!(
            !discovery.candidates().iter().any(|p| p.starts_with(&host)),
            "host home {host:?} leaked into the candidates"
        );
    }
}

/// A `MapEnv` models a closed set, so unknown keys are unset.
#[test]
fn map_env_reports_absent_keys_as_unset() {
    let env = MapEnv::new().with_var("PRESENT", "1");
    assert!(env.get("XDG_CONFIG_HOME").is_none());
    assert!(env.home_fallback().is_none());
}
