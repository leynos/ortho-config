//! Regression coverage for scoped discovery and file-layer policies.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Result, anyhow, ensure};
use ortho_config::{
    AutomaticMode, ConfigDiscovery, ConfigFilePolicy, ConfigPathSelector, DiscoveryLayersOutcome,
    DiscoveryScope, ExplicitMode, FileLayerOutcome, MapEnv, MergeComposer, OrthoError,
};

#[path = "support/layer_assertions.rs"]
mod layer_assertions;
#[path = "support/scoped_fixtures.rs"]
mod scoped_fixtures;

use layer_assertions::{assert_layer_path, merge_layers};
use scoped_fixtures::{write_body, write_config};

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

/// A three-rung selector chain whose environment rungs resolve from `source`.
///
/// The first rung is an empty CLI rung, which must lose to a populated
/// environment rung rather than reporting a missing file. `PRIMARY` is marked a
/// legacy alias and `SECONDARY` carries its own diagnostics label, so the
/// winner's metadata distinguishes the two rungs.
fn three_rung_policy(source: MapEnv) -> ConfigFilePolicy {
    three_rung_policy_with(source, None)
}

/// [`three_rung_policy`] with the CLI rung populated instead of empty.
///
/// A populated CLI rung is what makes rung *order* rather than rung *content*
/// decide the winner, so the precedence claim needs this spelling; the empty
/// one above would pass whether or not the chain were ordered at all.
fn three_rung_policy_with(source: MapEnv, cli: Option<&Path>) -> ConfigFilePolicy {
    ConfigFilePolicy::from_builder(
        ConfigDiscovery::builder("demo")
            .clear_project_roots()
            .env_source(Arc::new(source)),
    )
    .selectors([
        ConfigPathSelector::cli(cli.map(Path::to_path_buf)),
        ConfigPathSelector::env("PRIMARY").legacy_alias(),
        ConfigPathSelector::env("SECONDARY").label("secondary"),
    ])
}

/// Resolution stops at the first rung that yields a path.
///
/// Each scenario sets exactly one variable, so a regression to "last rung
/// wins", or to an unlabelled winner, shows up as a mismatch: the loser is
/// always a different rung from the one that must report.
#[test]
fn selector_precedence_carries_label_and_legacy_metadata() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let selected = temp.path().join("selected.toml");
    write_config(&selected, 7)?;

    // Only the last rung has a value, so the empty CLI rung and the unset
    // `PRIMARY` rung are both skipped and the labelled rung wins.
    let labelled =
        three_rung_policy(MapEnv::new().with_var("SECONDARY", &selected)).resolve_layers();
    let labelled_selection = labelled
        .selection()
        .ok_or_else(|| anyhow!("the SECONDARY rung must win when only it is set"))?;
    ensure!(
        labelled_selection.label() == "secondary",
        "the winning rung's own label must travel, got {}",
        labelled_selection.label()
    );
    ensure!(
        !labelled_selection.legacy(),
        "a labelled rung is not a legacy alias"
    );
    ensure!(
        labelled_selection.path() == selected.as_path(),
        "the winning rung must report the path it resolved"
    );

    // Only the middle rung has a value, so the alias rung wins instead.
    let alias = three_rung_policy(MapEnv::new().with_var("PRIMARY", &selected)).resolve_layers();
    let alias_selection = alias
        .selection()
        .ok_or_else(|| anyhow!("the PRIMARY rung must win when only it is set"))?;
    ensure!(
        alias_selection.legacy(),
        "PRIMARY was marked a legacy alias, so the winner must report it"
    );
    ensure!(
        alias_selection.label() == "PRIMARY",
        "legacy_alias must not disturb the label env() materialised, got {}",
        alias_selection.label()
    );

    // Both the CLI rung and the first environment rung have a value, so this is
    // the only scenario here decided by rung *order* rather than by which rung
    // happens to be populated. The legacy path covers the same rule in
    // `clap_integration/config_path.rs`; this pins it for the policy API.
    let other = temp.path().join("other.toml");
    write_config(&other, 9)?;
    let both = three_rung_policy_with(
        MapEnv::new().with_var("PRIMARY", &other),
        Some(selected.as_path()),
    )
    .resolve_layers();
    let both_selection = both
        .selection()
        .ok_or_else(|| anyhow!("a populated rung must resolve"))?;
    ensure!(
        both_selection.path() == selected.as_path(),
        "the CLI rung precedes every environment rung, so it must win"
    );
    Ok(())
}

/// A policy selecting `path` under `mode`, with no automatic fallbacks.
fn selecting_policy(path: &Path, mode: ExplicitMode) -> ConfigFilePolicy {
    ConfigFilePolicy::from_builder(ConfigDiscovery::builder("demo").clear_project_roots())
        .selectors([ConfigPathSelector::cli(Some(path.to_path_buf()))])
        .explicit_mode(mode)
}

/// An unparseable file is reported, not swallowed, and never fatal when optional.
///
/// The existing optional-selection test covers only a *missing* file, which
/// takes the `Ok(None)` arm. This covers the malformed file, which takes the
/// `Err` arm — the one that routes between `reportable_errors` and
/// `selected_error` purely on the explicit mode.
#[test]
fn optional_malformed_selection_is_reportable_but_required_malformed_is_fatal() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let malformed = temp.path().join("malformed.toml");
    write_body(&malformed, "value = ???\n")?;

    let optional = selecting_policy(&malformed, ExplicitMode::Optional).resolve_layers();
    ensure!(
        optional.reportable_errors().len() == 1,
        "an optional malformed file must be reported once, got {}",
        optional.reportable_errors().len()
    );
    ensure!(
        optional.selected_error().is_none(),
        "a reportable defect must not be recorded as a fatal selected error"
    );

    let mut errors = Vec::new();
    let layers = optional.into_layers_and_errors(&mut errors);
    ensure!(
        layers.is_empty(),
        "a file that cannot be parsed contributes no layer, got {}",
        layers.len()
    );
    ensure!(
        errors.len() == 1,
        "its defect must still reach the caller's error list, got {}",
        errors.len()
    );

    // The aggregating route must not silently succeed either: a reportable
    // error is only non-fatal when some layer loaded, and none did here.
    ensure!(
        selecting_policy(&malformed, ExplicitMode::Optional)
            .resolve_layers()
            .into_result()
            .is_err(),
        "an unreadable optional selection must aggregate its reportable error"
    );

    let required = selecting_policy(&malformed, ExplicitMode::RequiredExclusive).resolve_layers();
    ensure!(
        required.selected_error().is_some(),
        "a required explicit selection that cannot load must fail closed"
    );
    ensure!(
        required.reportable_errors().is_empty(),
        "a fatal selection is not also reported as a non-fatal defect"
    );
    Ok(())
}

#[test]
fn policy_project_root_drives_origins_and_scalar_preview() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let user_home = temp.path().join("user");
    let project = temp.path().join("project");
    write_body(
        &project.join("project.toml"),
        "value = 5\nitems = [1, 2]\n\n[nested]\ndepth = 1\n",
    )?;

    // The scope order names `User` before `Project`, but only the project file
    // exists: an `origins` that echoed the request rather than the report would
    // name both. `project_root` replaces the builder's roots, so the working
    // directory cannot contribute either.
    let mut outcome = ConfigFilePolicy::from_builder(
        ConfigDiscovery::builder("demo")
            .config_file_name("config.toml")
            .project_file_name("project.toml")
            .env_source(Arc::new(
                MapEnv::new().with_var("XDG_CONFIG_HOME", &user_home),
            )),
    )
    .project_root(&project)
    .automatic_mode(AutomaticMode::StackScopes)
    .scope_order([DiscoveryScope::User, DiscoveryScope::Project])
    .resolve_layers();

    ensure!(
        outcome.origins() == [DiscoveryScope::Project].as_slice(),
        "only the scope that contributed a layer is an origin, got {:?}",
        outcome.origins()
    );

    let preview = outcome.merged_file_value();
    ensure!(
        preview.get("value") == Some(&serde_json::json!(5)),
        "the scalar key must survive the preview, got {preview}"
    );
    ensure!(
        preview.get("nested").is_none() && preview.get("items").is_none(),
        "the preview is scalar-only, got {preview}"
    );

    let mut composer = MergeComposer::new();
    outcome.push_into(&mut composer);
    let layers = composer.layers();
    ensure!(
        layers.len() == 1,
        "the project file must reach the composer once, got {}",
        layers.len()
    );
    let merged = merge_layers(layers);
    ensure!(
        merged.get("nested").is_some() && merged.get("items").is_some(),
        "push_into carries the whole file, not the scalar preview, got {merged}"
    );
    Ok(())
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
