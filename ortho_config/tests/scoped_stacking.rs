//! Regression coverage for `AutomaticMode::StackScopes` multi-file composition.
//!
//! These cases exist because `StackScopes` once stopped at the first successful
//! candidate *per scope*, so only the most-preferred location in each scope
//! loaded and every lower-preferred file was silently ignored. Each test pins
//! one half of the corrected contract: every applicable file contributes, and
//! within a scope the historic preference order still decides the winner.

use anyhow::{Result, anyhow, ensure};
use ortho_config::{AutomaticMode, ConfigDiscovery, DiscoveryScope, MapEnv};

#[path = "support/layer_assertions.rs"]
mod layer_assertions;
#[path = "support/scoped_fixtures.rs"]
mod scoped_fixtures;

use layer_assertions::{assert_layer_path, merge_layers};
use scoped_fixtures::{write_body, write_config};

/// A user-scope discovery over `XDG_CONFIG_HOME` and `HOME`.
///
/// Both are set, so the user scope offers three locations in the documented
/// preference order: `$XDG_CONFIG_HOME/demo/config.toml`, then
/// `$HOME/.config/demo/config.toml`, then the `$HOME/.demo.toml` dotfile.
fn user_scope_discovery(user_home: &std::path::Path, home: &std::path::Path) -> ConfigDiscovery {
    ConfigDiscovery::builder("demo")
        .config_file_name("config.toml")
        .clear_project_roots()
        .env_source(std::sync::Arc::new(
            MapEnv::new()
                .with_var("XDG_CONFIG_HOME", user_home)
                .with_var("HOME", home),
        ))
        .build()
}

/// A scope loads every location that exists, not only its most-preferred one.
///
/// The count is asserted before the winning value, because a single surviving
/// layer would still produce the right `value` for the wrong reason.
#[test]
fn scope_stacking_loads_every_applicable_location() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let home = temp.path().join("home");
    let user_home = temp.path().join("user");
    write_config(&user_home.join("demo/config.toml"), 1)?;
    write_config(&home.join(".config/demo/config.toml"), 2)?;
    write_config(&home.join(".demo.toml"), 3)?;

    let outcome = user_scope_discovery(&user_home, &home)
        .compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User]);

    ensure!(outcome.required_errors.is_empty());
    ensure!(outcome.optional_errors.is_empty());
    ensure!(
        outcome.value.len() == 3,
        "all three user-scope locations must load, got {} layer(s)",
        outcome.value.len()
    );
    Ok(())
}

/// Within one scope the candidate list remains a preference order.
///
/// Without the reversal described in `discovery::scoped`, the dotfile would be
/// applied last and `value` would be 3 instead of 1.
#[test]
fn scope_stacking_keeps_the_most_preferred_location_winning() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let home = temp.path().join("home");
    let user_home = temp.path().join("user");
    write_config(&user_home.join("demo/config.toml"), 1)?;
    write_config(&home.join(".config/demo/config.toml"), 2)?;
    write_config(&home.join(".demo.toml"), 3)?;

    let outcome = user_scope_discovery(&user_home, &home)
        .compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User]);

    let layers = outcome.value;
    let winner = layers
        .last()
        .ok_or_else(|| anyhow!("the user scope must contribute layers"))?;
    assert_layer_path(winner, &user_home.join("demo/config.toml"))?;
    ensure!(
        merge_layers(layers).get("value") == Some(&serde_json::json!(1)),
        "the most-preferred user location must win"
    );
    Ok(())
}

/// A lower-preferred file still contributes the keys the winner does not set.
///
/// The point of stacking is that a fallback is not dead weight: it supplies a
/// base the winner refines. A per-key assertion is the only way to show that,
/// since the winner's keys mask the rest under a flat merge.
#[test]
fn scope_stacking_layers_lower_preference_keys_under_the_winner() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let home = temp.path().join("home");
    let user_home = temp.path().join("user");
    write_body(&user_home.join("demo/config.toml"), "value = 1\n")?;
    write_body(&home.join(".demo.toml"), "value = 3\nfallback_only = 7\n")?;

    let outcome = user_scope_discovery(&user_home, &home)
        .compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User]);

    ensure!(
        outcome.value.len() == 2,
        "both user-scope locations must load, got {} layer(s)",
        outcome.value.len()
    );
    let merged = merge_layers(outcome.value);
    ensure!(
        merged.get("value") == Some(&serde_json::json!(1)),
        "the winner's key must survive"
    );
    ensure!(
        merged.get("fallback_only") == Some(&serde_json::json!(7)),
        "the lower-preferred file's unique key must still contribute"
    );
    Ok(())
}

/// A malformed lower-preferred file is reported without losing the winner.
///
/// Stacking attempts every candidate, so it now opens files the first-wins scan
/// never reached. Their defects must be diagnosed rather than swallowed, and
/// must not stop the layers that did load — hence both assertions together.
#[test]
fn scope_stacking_reports_a_failed_lower_preference_candidate() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let home = temp.path().join("home");
    let user_home = temp.path().join("user");
    write_config(&user_home.join("demo/config.toml"), 1)?;
    write_body(&home.join(".demo.toml"), "value = {")?;

    let outcome = user_scope_discovery(&user_home, &home)
        .compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User]);

    ensure!(
        outcome.value.len() == 1,
        "the valid winner must still load, got {} layer(s)",
        outcome.value.len()
    );
    ensure!(
        outcome.optional_errors.len() == 1,
        "the malformed optional candidate must be reported once, got {}",
        outcome.optional_errors.len()
    );
    Ok(())
}

/// Extends-chain order is preserved inside each stacked location.
///
/// A scope now contributes several files, so the guarantee that a parent is
/// applied before its child has to hold per file, not merely per scope. The
/// child overrides the parent while the parent keeps a key the child omits,
/// which no reordering can satisfy by accident.
#[test]
fn scope_stacking_keeps_extends_parents_under_their_children() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let user_home = temp.path().join("user");
    write_body(
        &user_home.join("demo/base.toml"),
        "value = 1\nparent_only = 5\n",
    )?;
    // `extends` is resolved relative to the extending file, so the child names
    // its parent by bare name and both live in the same directory.
    write_body(
        &user_home.join("demo/config.toml"),
        "extends = \"base.toml\"\nvalue = 2\n",
    )?;

    let outcome = ConfigDiscovery::builder("demo")
        .config_file_name("config.toml")
        .clear_project_roots()
        .env_source(std::sync::Arc::new(
            MapEnv::new().with_var("XDG_CONFIG_HOME", &user_home),
        ))
        .build()
        .compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User]);

    ensure!(
        outcome.value.len() == 2,
        "the chain must contribute both files, got {} layer(s)",
        outcome.value.len()
    );
    let parent = outcome
        .value
        .first()
        .ok_or_else(|| anyhow!("the parent layer must come first"))?;
    assert_layer_path(parent, &user_home.join("demo/base.toml"))?;
    let child = outcome
        .value
        .last()
        .ok_or_else(|| anyhow!("the child layer must come last"))?;
    assert_layer_path(child, &user_home.join("demo/config.toml"))?;

    let merged = merge_layers(outcome.value);
    ensure!(
        merged.get("value") == Some(&serde_json::json!(2)),
        "the child must override the parent's key"
    );
    ensure!(
        merged.get("parent_only") == Some(&serde_json::json!(5)),
        "the parent's unique key must survive"
    );
    Ok(())
}

/// A file reachable from two scopes through different spellings loads once.
///
/// The project root here is `user/nested/..`, which names the same directory as
/// `user` but spells it differently. Candidate *assembly* keys on the literal
/// `OsString` (see `ConfigDiscovery::dedup_key`), so both spellings survive and
/// both reach the loader; only canonical-path filtering collapses them. That
/// makes this the case which actually exercises the filter — spellings that are
/// byte-identical would be removed at assembly, and the test would pass even
/// with the filter deleted.
///
/// Deduplication retains the earliest application position, which is why the
/// surviving layer is the user-scope one.
#[test]
fn scope_stacking_deduplicates_across_path_spellings() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let user_home = temp.path().join("user");
    // `nested` must exist for the `..` component to resolve.
    write_body(&user_home.join("nested/.keep"), "")?;
    write_body(&user_home.join(".demo.toml"), "value = 1\n")?;
    let project_root = user_home.join("nested/..");

    let outcome = ConfigDiscovery::builder("demo")
        .config_file_name("config.toml")
        .project_file_name(".demo.toml")
        .clear_project_roots()
        .add_project_root(&project_root)
        .env_source(std::sync::Arc::new(
            MapEnv::new().with_var("HOME", &user_home),
        ))
        .build()
        .compose_scoped_layers(
            AutomaticMode::StackScopes,
            &[DiscoveryScope::User, DiscoveryScope::Project],
        );

    ensure!(
        outcome.value.len() == 1,
        "one file reachable under two spellings must contribute one layer, got {}",
        outcome.value.len()
    );
    assert_layer_path(
        outcome
            .value
            .first()
            .ok_or_else(|| anyhow!("the shared file must contribute a layer"))?,
        &user_home.join(".demo.toml"),
    )
}
