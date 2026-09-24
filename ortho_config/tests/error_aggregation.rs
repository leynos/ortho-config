//! Tests for aggregated error reporting across configuration sources.
use anyhow::{Context as _, Result, anyhow, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use ortho_config::{MapEnv, OrthoConfig, OrthoError, SharedEnvSource, SharedScanEnvSource};
use rstest::rstest;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, OrthoConfig)]
struct AggConfig {
    #[expect(
        dead_code,
        reason = "Field is read via deserialization only in this test"
    )]
    port: u32,
}

#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(
    prefix = "AGG_",
    discovery(
        app_name = "agg_config",
        env_var = "AGG_CONFIG_PATH",
        dotfile_name = ".agg.toml"
    )
)]
struct DiscoveryErrorConfig {
    #[ortho_config(default = 0)]
    port: u32,
}

#[rstest]
fn aggregates_cli_file_env_errors() -> Result<()> {
    let fixture = tempfile::tempdir().context("create aggregation fixture directory")?;
    let fixture_root = Utf8Path::from_path(fixture.path())
        .ok_or_else(|| anyhow!("temporary fixture path is not UTF-8"))?;
    let config_path = fixture_root.join(".config.toml");
    let cap = Dir::open_ambient_dir(fixture_root, ambient_authority())
        .context("open aggregation fixture directory")?;
    cap.write(".config.toml", b"port = ")
        .context("write invalid configuration fixture")?;

    let source = Arc::new(
        MapEnv::new()
            .with_var("CONFIG_PATH", config_path.as_os_str())
            .with_var("PORT", "notanumber"),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let err = match AggConfig::load_from_iter_with_sources(["prog", "--bogus"], discovery, merge) {
        Ok(cfg) => return Err(anyhow!("expected aggregated error, got config {cfg:?}")),
        Err(err) => err,
    };
    let agg = match &*err {
        OrthoError::Aggregate(agg) => agg,
        other => return Err(anyhow!("unexpected error variant: {other:?}")),
    };
    let actual = agg.len();
    ensure!(
        actual == 3,
        "expected three aggregated errors, got {actual}"
    );
    let mut kinds = agg
        .iter()
        .map(|e| match e {
            OrthoError::CliParsing(_) => Ok(1),
            OrthoError::File { .. } => Ok(2),
            OrthoError::Merge { .. } | OrthoError::Gathering(_) => Ok(3),
            other => Err(anyhow!("unexpected aggregated error variant: {other:?}")),
        })
        .collect::<Result<Vec<_>>>()?;
    kinds.sort_unstable();
    ensure!(kinds == vec![1, 2, 3], "unexpected error kinds: {kinds:?}");
    Ok(())
}

#[rstest]
fn discovery_errors_hidden_when_fallback_succeeds() -> Result<()> {
    let fixture = tempfile::tempdir().context("create discovery fixture directory")?;
    let fixture_root = Utf8Path::from_path(fixture.path())
        .ok_or_else(|| anyhow!("temporary fixture path is not UTF-8"))?;
    let invalid_path = fixture_root.join("invalid.toml");
    let cap = Dir::open_ambient_dir(fixture_root, ambient_authority())
        .context("open discovery fixture directory")?;
    cap.write("invalid.toml", b"port = ???")
        .context("write invalid selector fixture")?;
    cap.write(".agg.toml", b"port = 7000")
        .context("write fallback configuration fixture")?;

    let source = Arc::new(
        MapEnv::new()
            .with_var("AGG_CONFIG_PATH", invalid_path.as_os_str())
            .with_var("XDG_CONFIG_HOME", fixture_root.as_os_str()),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let cfg = DiscoveryErrorConfig::load_from_iter_with_sources(["prog"], discovery, merge)
        .map_err(|err| anyhow!(err))?;
    let actual = cfg.port;
    ensure!(actual == 7000, "expected port 7000, got {actual}");
    Ok(())
}

#[rstest]
fn required_path_errors_surface_even_with_fallback() -> Result<()> {
    let fixture = tempfile::tempdir().context("create required-path fixture directory")?;
    let fixture_root = Utf8Path::from_path(fixture.path())
        .ok_or_else(|| anyhow!("temporary fixture path is not UTF-8"))?;
    let missing_path = fixture_root.join("missing.toml");
    let cap = Dir::open_ambient_dir(fixture_root, ambient_authority())
        .context("open required-path fixture directory")?;
    cap.write(".agg.toml", b"port = 7000")
        .context("write fallback configuration fixture")?;

    let source = Arc::new(MapEnv::new().with_var("XDG_CONFIG_HOME", fixture_root.as_os_str()));
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let err = match DiscoveryErrorConfig::load_from_iter_with_sources(
        ["prog", "--config-path", missing_path.as_str()],
        discovery,
        merge,
    ) {
        Ok(cfg) => return Err(anyhow!("expected missing config path error, got {cfg:?}")),
        Err(err) => err,
    };
    ensure!(
        matches!(&*err, OrthoError::File { .. }),
        "expected file error, got {err:?}"
    );
    Ok(())
}

#[rstest]
fn discovery_errors_surface_when_all_candidates_fail() -> Result<()> {
    let fixture = tempfile::tempdir().context("create failed-discovery fixture directory")?;
    let fixture_root = Utf8Path::from_path(fixture.path())
        .ok_or_else(|| anyhow!("temporary fixture path is not UTF-8"))?;
    let invalid_path = fixture_root.join("invalid.toml");
    let cap = Dir::open_ambient_dir(fixture_root, ambient_authority())
        .context("open failed-discovery fixture directory")?;
    cap.write("invalid.toml", b"port = ???")
        .context("write invalid selector fixture")?;

    let source = Arc::new(
        MapEnv::new()
            .with_var("AGG_CONFIG_PATH", invalid_path.as_os_str())
            .with_var("XDG_CONFIG_HOME", fixture_root.as_os_str())
            .with_var("XDG_CONFIG_DIRS", fixture_root.as_os_str()),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;
    let err = match DiscoveryErrorConfig::load_from_iter_with_sources(["prog"], discovery, merge) {
        Ok(cfg) => return Err(anyhow!("expected discovery error, got {cfg:?}")),
        Err(err) => err,
    };
    ensure!(
        matches!(&*err, OrthoError::File { .. }),
        "expected file error, got {err:?}"
    );
    Ok(())
}
