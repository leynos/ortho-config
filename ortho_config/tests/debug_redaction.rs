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

/// The config-path selector name, kept distinct from [`SECRET_KEY`].
///
/// Discovery prints the selector name deliberately: it is a caller-chosen
/// identifier, not a datum read from the environment. Separating it from the
/// injected source's key lets the tests below assert that no key belonging to
/// the source reaches the rendered output.
const SELECTOR_VAR: &str = "DEMO_CONFIG_PATH";

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
        .env_var(SELECTOR_VAR)
        .add_explicit_path(SECRET_PATH)
        .env_source(Arc::new(secret_env));
    let rendered = format!("{builder:?}");
    assert!(
        !rendered.contains(SECRET_KEY),
        "builder debug output leaked an environment key: {rendered}"
    );
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
        .env_var(SELECTOR_VAR)
        .add_explicit_path(SECRET_PATH)
        .env_source(Arc::new(secret_env))
        .build();
    let rendered = format!("{discovery:?}");
    assert!(
        !rendered.contains(SECRET_KEY),
        "discovery debug output leaked an environment key: {rendered}"
    );
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

// The `ConfigDiscovery` Debug snapshot lives with the unit tests
// (`discovery::tests::project_root::discovery_debug_snapshot`): it injects
// the crate-internal project-root resolver so the counted root is a fixed
// value rather than the host's working directory.
