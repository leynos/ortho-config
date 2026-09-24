//! Regression coverage for scoped discovery and file-layer policies.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow, ensure};
use ortho_config::{
    AutomaticMode, ConfigDiscovery, ConfigFilePolicy, ConfigPathSelector, DiscoveryLayersOutcome,
    DiscoveryScope, ExplicitMode, FileLayerOutcome, MapEnv, OrthoError,
};

#[path = "support/layer_assertions.rs"]
mod layer_assertions;
#[path = "support/scoped_fixtures.rs"]
mod scoped_fixtures;

use layer_assertions::{assert_layer_path, merge_layers};
use scoped_fixtures::write_config;

/// Build a discovery whose user scope is `user_home` and project scope `project`.
fn scoped_discovery(user_home: &Path, project: &Path) -> ConfigDiscovery {
    ConfigDiscovery::builder("demo")
        .config_file_name("config.toml")
        .project_file_name("project.toml")
        .clear_project_roots()
        .add_project_root(project)
        .env_source(Arc::new(
            MapEnv::new().with_var("XDG_CONFIG_HOME", user_home),
        ))
        .build()
}

#[test]
fn stack_scopes_places_project_layers_after_user_layers() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let user_home = temp.path().join("user");
    let project = temp.path().join("project");
    write_config(&user_home.join("demo/config.toml"), 1)?;
    write_config(&project.join("project.toml"), 2)?;

    let outcome = scoped_discovery(&user_home, &project).compose_scoped_layers(
        AutomaticMode::StackScopes,
        &[DiscoveryScope::User, DiscoveryScope::Project],
    );
    ensure!(outcome.required_errors.is_empty());
    ensure!(outcome.optional_errors.is_empty());
    ensure!(
        merge_layers(outcome.value).get("value") == Some(&serde_json::json!(2)),
        "project value must override user value"
    );
    Ok(())
}

#[test]
fn selected_path_suppresses_automatic_scopes() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let user_home = temp.path().join("user");
    let project = temp.path().join("project");
    let selected = temp.path().join("selected.toml");
    write_config(&user_home.join("demo/config.toml"), 1)?;
    write_config(&project.join("project.toml"), 2)?;
    write_config(&selected, 3)?;

    let policy = ConfigFilePolicy::from_builder(
        ConfigDiscovery::builder("demo")
            .config_file_name("config.toml")
            .project_file_name("project.toml")
            .clear_project_roots()
            .add_project_root(&project)
            .env_source(Arc::new(
                MapEnv::new().with_var("XDG_CONFIG_HOME", &user_home),
            )),
    )
    .selectors([ConfigPathSelector::cli(Some(selected.clone()))])
    .automatic_mode(AutomaticMode::StackScopes)
    .scope_order([DiscoveryScope::User, DiscoveryScope::Project]);

    let layers = policy.resolve_layers().into_result()?;
    ensure!(
        layers.len() == 1,
        "selection must suppress automatic layers"
    );
    let first = layers
        .first()
        .ok_or_else(|| anyhow!("selection must yield the selected layer"))?;
    assert_layer_path(first, &selected)?;
    // The path assertion alone would still pass if the selected file loaded but
    // its value lost to a suppressed automatic layer, so pin the value too.
    ensure!(
        merge_layers(layers).get("value") == Some(&serde_json::json!(3)),
        "the selected file's value must survive"
    );
    Ok(())
}

#[test]
fn optional_selected_path_does_not_report_a_missing_file() {
    let policy =
        ConfigFilePolicy::from_builder(ConfigDiscovery::builder("demo").clear_project_roots())
            .selectors([ConfigPathSelector::cli(Some(PathBuf::from("missing.toml")))])
            .explicit_mode(ExplicitMode::Optional);
    assert!(policy.resolve_layers().into_result().is_ok());
}

#[test]
fn required_selected_path_reports_a_missing_file() {
    let policy =
        ConfigFilePolicy::from_builder(ConfigDiscovery::builder("demo").clear_project_roots())
            .selectors([ConfigPathSelector::cli(Some(PathBuf::from("missing.toml")))])
            .explicit_mode(ExplicitMode::RequiredExclusive);
    assert!(policy.resolve_layers().into_result().is_err());
}

#[test]
fn scoped_loading_deduplicates_a_file_across_scopes() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().join("shared");
    write_config(&root.join("demo/config.toml"), 1)?;

    let outcome = ConfigDiscovery::builder("demo")
        .config_file_name("config.toml")
        .project_file_name("config.toml")
        .clear_project_roots()
        .add_project_root(root.join("demo"))
        .env_source(Arc::new(MapEnv::new().with_var("XDG_CONFIG_HOME", &root)))
        .build()
        .compose_scoped_layers(
            AutomaticMode::StackScopes,
            &[DiscoveryScope::User, DiscoveryScope::Project],
        );
    ensure!(
        outcome.value.len() == 1,
        "one file reachable from two scopes must contribute one layer, got {}",
        outcome.value.len()
    );
    Ok(())
}

#[test]
fn discovery_outcome_lift_preserves_reportable_errors() {
    let error = Arc::new(OrthoError::Validation {
        key: String::from("test"),
        message: String::from("failure"),
    });
    let outcome: FileLayerOutcome = DiscoveryLayersOutcome {
        value: Vec::new(),
        required_errors: vec![error],
        optional_errors: Vec::new(),
    }
    .into();
    assert_eq!(outcome.reportable_errors().len(), 1);
}

#[test]
fn compose_layers_remains_first_wins() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let first = temp.path().join("first.toml");
    let user = temp.path().join("user");
    write_config(&first, 1)?;
    write_config(&user.join("demo/config.toml"), 2)?;
    let outcome = ConfigDiscovery::builder("demo")
        .add_explicit_path(&first)
        .clear_project_roots()
        .env_source(Arc::new(MapEnv::new().with_var("XDG_CONFIG_HOME", user)))
        .build()
        .compose_layers();
    ensure!(outcome.value.len() == 1);
    let first_layer = outcome
        .value
        .first()
        .ok_or_else(|| anyhow!("the explicit candidate must yield a layer"))?;
    assert_layer_path(first_layer, &first)?;
    // First-wins means the explicit candidate's value survives, not merely that
    // its path was recorded; a regression to "load everything" would show here.
    ensure!(
        merge_layers(outcome.value).get("value") == Some(&serde_json::json!(1)),
        "the first explicit candidate's value must win"
    );
    Ok(())
}
