//! Regression tests for secret redaction in `Debug` output.
//!
//! `MapEnv` frequently holds secret-shaped fixtures, `EnvSource` requires
//! `Debug`, and discovery types hold the source, so a derived implementation
//! anywhere in that chain would print secrets wherever a value is logged or
//! unwrapped. Each case formats with `format!("{value:?}")` and asserts the
//! distinctive fixtures never appear. Everything is injected; no test reads
//! or mutates the process environment.

use std::sync::Arc;

use ortho_config::{ConfigDiscovery, MapEnv};
use rstest::{fixture, rstest};

const SECRET_KEY: &str = "SEKRIT_DEBUG_KEY_7f3a";
const SECRET_VALUE: &str = "sekrit-debug-value-1c9e";
const SECRET_PATH: &str = "/sekrit/debug/path-55d1";

#[fixture]
fn secret_env() -> MapEnv {
    MapEnv::new().with_var(SECRET_KEY, SECRET_VALUE)
}

#[rstest]
fn map_env_debug_reveals_neither_keys_nor_values(secret_env: MapEnv) {
    let rendered = format!("{secret_env:?}");
    assert!(
        !rendered.contains(SECRET_KEY),
        "MapEnv debug output leaked the key: {rendered}"
    );
    assert!(
        !rendered.contains(SECRET_VALUE),
        "MapEnv debug output leaked the value: {rendered}"
    );
    assert!(
        rendered.contains("MapEnv"),
        "the type name should still identify the value: {rendered}"
    );
}

#[rstest]
fn builder_debug_reveals_no_secrets(secret_env: MapEnv) {
    let builder = ConfigDiscovery::builder("demo")
        .env_var(SECRET_KEY)
        .add_explicit_path(SECRET_PATH)
        .env_source(Arc::new(secret_env));
    let rendered = format!("{builder:?}");
    assert!(
        !rendered.contains(SECRET_VALUE),
        "builder debug output leaked an environment value: {rendered}"
    );
    assert!(
        !rendered.contains(SECRET_PATH),
        "builder debug output leaked a path: {rendered}"
    );
}

#[rstest]
fn discovery_debug_reveals_no_secrets(secret_env: MapEnv) {
    let discovery = ConfigDiscovery::builder("demo")
        .env_var(SECRET_KEY)
        .add_explicit_path(SECRET_PATH)
        .env_source(Arc::new(secret_env))
        .build();
    let rendered = format!("{discovery:?}");
    assert!(
        !rendered.contains(SECRET_VALUE),
        "discovery debug output leaked an environment value: {rendered}"
    );
    assert!(
        !rendered.contains(SECRET_PATH),
        "discovery debug output leaked a path: {rendered}"
    );
}

/// Pin the complete redacted `Debug` rendering of `MapEnv`.
///
/// The exact-string snapshot complements the secret-exclusion assertions
/// above: any new field added to the output must be reviewed here, so a
/// leak cannot arrive silently alongside a legitimate change.
#[rstest]
fn map_env_debug_snapshot(secret_env: MapEnv) {
    assert_eq!(format!("{secret_env:?}"), "MapEnv { vars: 1, .. }");
}

/// Pin the complete redacted `Debug` rendering of `ConfigDiscovery`.
///
/// Every shown value is a developer-chosen constant; paths and environment
/// values must never join them. The snapshot is path-free by construction,
/// so it is stable on every host.
#[rstest]
fn discovery_debug_snapshot(secret_env: MapEnv) {
    let discovery = ConfigDiscovery::builder("demo")
        .clear_project_roots()
        .env_source(Arc::new(secret_env))
        .build();
    // `project_roots: 1` is the default resolver's current directory, which
    // is counted but never printed; the rendering stays path-free.
    assert_eq!(
        format!("{discovery:?}"),
        "ConfigDiscovery { env_var: None, app_name: \"demo\", \
         config_file_name: \"config.toml\", dotfile_name: \".demo.toml\", \
         project_file_name: \".demo.toml\", explicit_paths: 0, \
         required_explicit_paths: 0, project_roots: 1, .. }"
    );
}
