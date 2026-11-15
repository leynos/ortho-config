//! Tests for the profile metadata in the documentation IR (decision D15).

use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use ortho_config::docs::{DocProfilesMeta, OrthoConfigDocs, SourceKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_", profiles)]
struct ProfileDocsConfig {
    #[serde(default)]
    retries: u32,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "APP_")]
struct LegacyDocsConfig {
    #[serde(default)]
    retries: u32,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "APP_",
    profiles,
    precedence(order = ["defaults", "file", "profiles", "env", "cli"])
)]
struct ExplicitPrecedenceDocsConfig {
    #[serde(default)]
    retries: u32,
}

#[test]
fn profile_enabled_struct_emits_selection_metadata() -> Result<()> {
    let profiles = ProfileDocsConfig::get_doc_metadata().profiles;
    let Some(meta) = profiles else {
        return Err(anyhow!("expected profile metadata for an opted-in struct"));
    };
    ensure!(
        meta == DocProfilesMeta {
            flag: String::from("profile"),
            env_var: String::from("APP_PROFILE"),
        },
        "unexpected profile metadata {meta:?}"
    );
    Ok(())
}

#[test]
fn legacy_struct_omits_profile_metadata() {
    assert!(LegacyDocsConfig::get_doc_metadata().profiles.is_none());
}

/// The precedence tiers, in the order the metadata reports them.
fn precedence_order(docs: &ortho_config::docs::DocMetadata) -> Vec<SourceKind> {
    docs.sections
        .precedence
        .as_ref()
        .map(|precedence| precedence.order.clone())
        .unwrap_or_default()
}

#[test]
fn profile_enabled_struct_emits_the_five_tier_precedence() {
    assert_eq!(
        precedence_order(&ProfileDocsConfig::get_doc_metadata()),
        vec![
            SourceKind::Defaults,
            SourceKind::File,
            SourceKind::Profile,
            SourceKind::Env,
            SourceKind::Cli,
        ],
    );
}

#[test]
fn legacy_struct_keeps_the_four_tier_precedence() {
    assert_eq!(
        precedence_order(&LegacyDocsConfig::get_doc_metadata()),
        vec![
            SourceKind::Defaults,
            SourceKind::File,
            SourceKind::Env,
            SourceKind::Cli,
        ],
    );
}

/// A user-written `precedence(order = [...])` resolves each tier through the
/// explicit-spelling parser rather than the automatic five-tier order, so this
/// is the only test that reaches the profile spellings at all.
#[test]
fn explicit_precedence_order_accepts_a_profile_spelling() {
    assert_eq!(
        precedence_order(&ExplicitPrecedenceDocsConfig::get_doc_metadata()),
        vec![
            SourceKind::Defaults,
            SourceKind::File,
            SourceKind::Profile,
            SourceKind::Env,
            SourceKind::Cli,
        ],
    );
}
