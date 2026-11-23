//! Regression coverage for required selectors in scoped composition.

use std::sync::Arc;

use anyhow::{Result, ensure};
use ortho_config::{AutomaticMode, ConfigDiscovery, DiscoveryScope, MapEnv};

/// A required selector with no automatic scope is reported by `StackScopes`.
///
/// `StackScopes` walks only automatic scopes, so an explicit selector would
/// otherwise be invisible to the composition: its missing file would produce no
/// error and the caller could silently fall back to the stacked files.
#[test]
fn stack_scopes_reports_a_missing_required_selector() -> Result<()> {
    let discovery = ConfigDiscovery::builder("demo")
        .clear_project_roots()
        .env_var("DEMO_CONFIG")
        .env_source(Arc::new(
            MapEnv::new().with_var("DEMO_CONFIG", "/nonexistent/selected.toml"),
        ))
        .build();

    let outcome =
        discovery.compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User]);

    ensure!(
        outcome.value.is_empty(),
        "the missing selector should contribute no layers"
    );
    ensure!(
        outcome.required_errors.len() == 1,
        "the missing selector should be one required error, got {}",
        outcome.required_errors.len()
    );
    ensure!(
        outcome.optional_errors.is_empty(),
        "the missing selector should not be optional"
    );
    Ok(())
}
