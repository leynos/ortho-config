//! Regression coverage for injected sources on the policy discovery path.
//!
//! A struct that opts into `discovery(automatic_mode = ...)` takes the policy
//! branch of the generated loader rather than the legacy one. That branch
//! assembles its own `ConfigDiscovery` and its own selector chain, so two
//! things have to travel with it that the legacy emitter installs by default:
//! the caller's injected environment source, and the config-path variable a
//! struct honours even when it never names one.
//!
//! Losing the first makes `load_from_iter_with_sources` silently read the real
//! process environment instead of the caller's map; losing the second makes the
//! policy form ignore the very variable the non-policy form consults. Neither
//! failure is visible from the generated code alone, so both are pinned here.
//!
//! The process environment is only ever *cleared* here, never set. A value can
//! therefore only come from the injected map, which is the property under test.
//! `Jail::expect_with` performs the clearing and restores the environment after
//! the closure returns.

use anyhow::{Context as _, Result, anyhow, ensure};
use figment::Jail;
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tempfile::TempDir;

/// Fixture writing shared with the scoped-discovery suites. Its `write_config`
/// stages `value = <n>` files through a capability handle rather than ambient
/// `std::fs`, which is the repository's filesystem policy.
#[path = "support/scoped_fixtures.rs"]
mod scoped_fixtures;

use scoped_fixtures::write_config;

/// A policy-enabled struct that never writes `env_var` of its own.
///
/// Its config-path variable is therefore the materialised default,
/// `POLICY_SRC_CONFIG_PATH`, which is exactly the selector the policy branch
/// has to install without being asked.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "POLICY_SRC_",
    discovery(
        app_name = "policy_src_app",
        config_file_name = "policy-src.toml",
        automatic_mode = "first_wins"
    )
)]
struct PolicySourcedConfig {
    #[ortho_config(default = 0)]
    value: u32,
}

/// Load through both injected capabilities.
fn load_from_map(source: Arc<MapEnv>) -> Result<PolicySourcedConfig> {
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    PolicySourcedConfig::load_from_iter_with_sources(["policy-src"], discovery, merge)
        .map_err(|error| anyhow!(error))
        .context("load policy-enabled configuration from injected sources")
}

/// The caller's map supplies the config-path variable, not the process env.
///
/// A scope is given no file at all, so the selection is the only route to a
/// layer. Had the policy branch dropped the injected source, the cleared
/// process environment would answer instead and `value` would fall back to its
/// default; had it dropped the default selector, the variable would never be
/// consulted and the sample scope would decide.
#[test]
fn policy_discovery_honours_an_injected_config_path_variable() -> Result<()> {
    let mut test_result = Ok(());
    Jail::expect_with(|jail| {
        jail.clear_env();
        test_result = (|| {
            let dir = TempDir::new().context("create policy fixture directory")?;
            let selected = dir.path().join("selected.toml");
            write_config(&selected, 4242)?;
            // An empty scope base keeps the selector the only source of a file.
            let empty_scope = dir.path().join("empty");

            let source = Arc::new(
                MapEnv::new()
                    .with_var("POLICY_SRC_CONFIG_PATH", &selected)
                    .with_var("XDG_CONFIG_HOME", &empty_scope)
                    .with_var("HOME", &empty_scope),
            );
            let config = load_from_map(source)?;

            ensure!(
                config.value == 4242,
                "the injected POLICY_SRC_CONFIG_PATH must select the file, got {}",
                config.value
            );
            Ok(())
        })();
        Ok(())
    });
    test_result
}

/// The caller's map also drives automatic scope discovery.
///
/// No selector variable is set, so the answer comes from the automatic path.
/// The scope base is named only in the injected map, and the process
/// environment is cleared, so a layer can only load if the policy branch passed
/// that map to the builder it constructs.
#[test]
fn policy_discovery_reads_scopes_from_an_injected_environment() -> Result<()> {
    let mut test_result = Ok(());
    Jail::expect_with(|jail| {
        jail.clear_env();
        test_result = (|| {
            let dir = TempDir::new().context("create policy fixture directory")?;
            let scope = dir.path().join("scope");
            write_config(&scope.join("policy_src_app/policy-src.toml"), 77)?;

            let source = Arc::new(
                MapEnv::new()
                    .with_var("XDG_CONFIG_HOME", &scope)
                    .with_var("HOME", &scope),
            );
            let config = load_from_map(source)?;

            ensure!(
                config.value == 77,
                "the injected XDG_CONFIG_HOME must supply the automatic layer, got {}",
                config.value
            );
            Ok(())
        })();
        Ok(())
    });
    test_result
}
