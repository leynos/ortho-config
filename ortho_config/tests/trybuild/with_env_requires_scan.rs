//! Trybuild fixture: a lookup-only source cannot back a `_with_env` loader.
//!
//! The loaders enumerate the environment layer, so the source must implement
//! `ScanEnvSource` as well as `EnvSource`. A lookup-only source is rejected at
//! compile time rather than silently reading nothing.

use ortho_config::{EnvSource, OrthoConfig};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::sync::Arc;

/// A lookup-only source: it deliberately cannot enumerate.
#[derive(Debug)]
struct LookupOnly;

impl EnvSource for LookupOnly {
    fn get(&self, _key: &str) -> Option<OsString> {
        None
    }
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "FIXED_")]
struct Config {
    #[ortho_config(default = 1)]
    port: u16,
}

fn main() {
    let _ = Config::load_from_iter_with_env(["fixed"], Arc::new(LookupOnly));
}
