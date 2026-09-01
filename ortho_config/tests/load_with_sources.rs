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
            .with_var("WHOLE_CONFIG_JOBS", "7"),
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
    Ok(())
}
