//! Trybuild fixture: a downstream `ScanEnvSource` implementation.
//!
//! `ScanEnvSource` intentionally has a distinct trait object from `EnvSource`.
//! This confirms that downstream callers can opt into the enumerating port
//! without widening discovery's lookup-only contract.

use ortho_config::{ScanEnvSource, SharedScanEnvSource};
use std::ffi::OsString;
use std::sync::Arc;

/// Enumerates a fixed source without reading the process environment.
#[derive(Debug)]
struct FixedScanEnv;

impl ScanEnvSource for FixedScanEnv {
    fn scan(&self) -> Vec<(OsString, OsString)> {
        vec![(OsString::from("DEMO_HOST"), OsString::from("localhost"))]
    }
}

/// Fails to compile unless `ScanEnvSource` implementors satisfy the supertraits.
fn requires_debug_send_sync<T: std::fmt::Debug + Send + Sync>(value: T) -> T {
    value
}

fn main() {
    let source = requires_debug_send_sync(FixedScanEnv);

    // Unsized coercion to the trait object, then the documented alias.
    let shared: Arc<dyn ScanEnvSource> = Arc::new(source);
    let shared: SharedScanEnvSource = shared;

    assert_eq!(
        shared.scan(),
        vec![(OsString::from("DEMO_HOST"), OsString::from("localhost"))]
    );
}
