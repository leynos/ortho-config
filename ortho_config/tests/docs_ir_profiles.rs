//! Tests for the profile metadata in the documentation IR (decision D15).

use anyhow::{Result, anyhow, ensure};
use ortho_config::OrthoConfig;
use ortho_config::docs::{DocProfilesMeta, OrthoConfigDocs};
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
