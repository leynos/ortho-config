//! Regression coverage for scoped discovery and file-layer policies.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{
    AutomaticMode, ConfigDiscovery, ConfigFilePolicy, ConfigPathSelector, DiscoveryLayersOutcome,
    DiscoveryScope, ExplicitMode, FileLayerOutcome, MapEnv, OrthoError, declarative::merge_value,
};

fn write_config(path: &Path, value: u32) -> Result<()> {
    let root = Dir::open_ambient_dir(Path::new("/"), ambient_authority())?;
    let relative = path
        .strip_prefix("/")
        .map_err(|_| anyhow!("fixture path must be absolute"))?;
    let parent = relative
        .parent()
        .ok_or_else(|| anyhow!("test fixture path has no parent: {}", path.display()))?;
    root.create_dir_all(parent)?;
    root.write(relative, format!("value = {value}\n"))?;
    Ok(())
}

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
    let mut merged = serde_json::Value::Null;
    for layer in outcome.value {
        merge_value(&mut merged, layer.into_value());
    }
    ensure!(
        merged.get("value") == Some(&serde_json::json!(2)),
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
    ensure!(
        layers
            .first()
            .and_then(ortho_config::MergeLayer::path)
            .is_some_and(|path| path.as_std_path() == selected)
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
    ensure!(outcome.value.len() == 1);
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
    ensure!(
        outcome
            .value
            .first()
            .and_then(ortho_config::MergeLayer::path)
            .is_some_and(|path| path.as_std_path() == first)
    );
    Ok(())
}
