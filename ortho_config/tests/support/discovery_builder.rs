//! The discovery instance the injected-source suites are all built around.
//!
//! Four suites — candidates, properties, telemetry and metrics — each need a
//! `ConfigDiscovery` configured identically: the same application name, the
//! same selector variable, the platform project roots cleared, and one
//! deterministic project root in their place. Four copies of that builder drift
//! independently, and a suite quietly testing a differently-configured
//! discovery is a suite whose disagreement with the others means nothing.
//!
//! Sharing this does not weaken the isolation those suites document. The
//! helper is a pure builder: it holds no state, mutates nothing, and touches
//! neither the process environment nor the filesystem. Each call returns a
//! fresh `ConfigDiscovery` over the caller's own [`MapEnv`], so cases remain
//! independent and free to run concurrently.

use ortho_config::{ConfigDiscovery, MapEnv};
use std::path::Path;
use std::sync::Arc;

/// The application name used across the injected-discovery suites.
pub const APP: &str = "demo";
/// The selector variable name used across the injected-discovery suites.
pub const SELECTOR: &str = "DEMO_CONFIG";
/// The single deterministic project root used across the suites.
pub const PROJECT_ROOT: &str = "/workspace";

/// Build a discovery reading `env` and nothing else the host provides.
///
/// `clear_project_roots` is what makes the result deterministic: without it the
/// discovered roots depend on the working directory of whichever process ran
/// the test.
#[must_use]
pub fn discovery_with(env: MapEnv) -> ConfigDiscovery {
    ConfigDiscovery::builder(APP)
        .env_var(SELECTOR)
        .clear_project_roots()
        .add_project_root(Path::new(PROJECT_ROOT))
        .env_source(Arc::new(env))
        .build()
}
