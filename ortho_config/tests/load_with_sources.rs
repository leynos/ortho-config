//! Complete derived configuration loading from injected environment sources.
//!
//! This test keeps discovery and merge data in one `MapEnv`. It must not mutate
//! the process environment: that would hide a failure to thread either source
//! through the derive-generated loading path.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig, SharedEnvSource, SharedScanEnvSource};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "WHOLE_CONFIG_",
    discovery(
        app_name = "whole-config",
        config_file_name = "whole-config.toml",
        dotfile_name = ".whole-config.toml",
        project_file_name = ".whole-config.toml"
    )
)]
struct WholeConfig {
    #[ortho_config(default = 1)]
    jobs: u8,
    from_file: String,
    #[ortho_config(skip_cli)]
    database: Database,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Database {
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
struct UnprefixedConfig {
    #[ortho_config(skip_cli)]
    database: Database,
}

/// Write a selector fixture through a capability handle.
fn write_selector_fixture(dir: &Path) -> Result<PathBuf> {
    let cap = Dir::open_ambient_dir(dir, ambient_authority())
        .context("open source-aware loading fixture directory")?;
    cap.write("selected.toml", b"from_file = \"selected\"\n")
        .context("write source-aware loading fixture")?;
    Ok(dir.join("selected.toml"))
}

/// One map drives both the selector lookup and the complete merge layer.
#[test]
fn derived_loading_uses_one_map_for_both_environment_capabilities() -> Result<()> {
    let fixture_dir = tempfile::tempdir().context("create source-aware loading fixture")?;
    let selector_path = write_selector_fixture(fixture_dir.path())?;
    let source = Arc::new(
        MapEnv::new()
            .with_var("WHOLE_CONFIG_CONFIG_PATH", &selector_path)
            .with_var("WHOLE_CONFIG_JOBS", "7")
            .with_var("WHOLE_CONFIG_DATABASE__HOST", "db.prefixed.test")
            .with_var("whole_config_database__port", "5432"),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;

    let config = WholeConfig::load_from_iter_with_sources(["whole-config"], discovery, merge)
        .map_err(|error| anyhow::anyhow!(error))
        .context("load complete configuration from injected sources")?;

    ensure!(
        config.jobs == 7,
        "expected injected jobs, got {}",
        config.jobs
    );
    ensure!(
        config.from_file == "selected",
        "expected selected file value, got {}",
        config.from_file
    );
    ensure!(
        config.database.host == "db.prefixed.test" && config.database.port == 5432,
        "expected injected prefixed nested database values"
    );
    Ok(())
}

/// Generated raw providers apply the same uppercase and split replay rules.
#[test]
fn unprefixed_derived_loading_replays_generated_key_transforms() -> Result<()> {
    let source = Arc::new(
        MapEnv::new()
            .with_var("DATABASE__HOST", "db.raw.test")
            .with_var("database__PORT", "15432"),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;

    let config =
        UnprefixedConfig::load_from_iter_with_sources(["unprefixed-config"], discovery, merge)
            .map_err(|error| anyhow::anyhow!(error))
            .context("load raw derived configuration from injected sources")?;

    ensure!(
        config.database.host == "db.raw.test" && config.database.port == 15432,
        "expected injected raw nested database values"
    );
    Ok(())
}
