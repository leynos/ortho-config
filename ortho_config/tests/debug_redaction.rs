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

const SECRET_KEY: &str = "SEKRIT_DEBUG_KEY_7f3a";
const SECRET_VALUE: &str = "sekrit-debug-value-1c9e";
const SECRET_PATH: &str = "/sekrit/debug/path-55d1";

fn secret_env() -> MapEnv {
    MapEnv::new().with_var(SECRET_KEY, SECRET_VALUE)
}

#[test]
fn map_env_debug_reveals_neither_keys_nor_values() {
    let rendered = format!("{:?}", secret_env());
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

#[test]
fn builder_debug_reveals_no_secrets() {
    let builder = ConfigDiscovery::builder("demo")
        .env_var(SECRET_KEY)
        .add_explicit_path(SECRET_PATH)
        .env_source(Arc::new(secret_env()));
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

#[test]
fn discovery_debug_reveals_no_secrets() {
    let discovery = ConfigDiscovery::builder("demo")
        .env_var(SECRET_KEY)
        .add_explicit_path(SECRET_PATH)
        .env_source(Arc::new(secret_env()))
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
