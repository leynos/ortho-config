//! Behavioural coverage for the injectable scanning environment port.

use ortho_config::{MapEnv, ScanEnvSource};
use std::collections::HashMap;
use std::ffi::OsString;

#[test]
fn map_env_scans_owned_key_value_pairs() {
    let env = MapEnv::new()
        .with_var("APP_HOST", "localhost")
        .with_var("APP_PORT", "8080");
    let scanned: HashMap<_, _> = env.scan().into_iter().collect();

    assert_eq!(scanned.len(), 2);
    assert_eq!(
        scanned.get(&OsString::from("APP_HOST")),
        Some(&OsString::from("localhost"))
    );
    assert_eq!(
        scanned.get(&OsString::from("APP_PORT")),
        Some(&OsString::from("8080"))
    );
}
