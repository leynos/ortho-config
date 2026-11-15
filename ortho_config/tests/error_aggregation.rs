//! Tests for aggregated error reporting across configuration sources.
use anyhow::{Context as _, Result, anyhow, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use ortho_config::{MapEnv, OrthoConfig, OrthoError, SharedEnvSource, SharedScanEnvSource};
use rstest::{fixture, rstest};
use serde::Deserialize;
use std::sync::Arc;

#[path = "support/isolated_env.rs"]
mod isolated_env;
use isolated_env::with_host_overrides;

/// One temporary root, its UTF-8 view, and a capability handle to it.
///
/// The `TempDir` field is never read; it is held so the directory outlives the
/// test that consumes the fixture.
struct Fixture {
    /// Keeps the temporary directory alive for the test's duration.
    _temp_dir: tempfile::TempDir,
    root: camino::Utf8PathBuf,
    dir: Dir,
}

/// Stage a fixture root for an aggregation case.
///
/// Consume `Result<Fixture>` with `?` so a failure to create the temporary
/// directory reports through the test's own error rather than panicking.
#[fixture]
fn fixture() -> Result<Fixture> {
    let guard = tempfile::tempdir().context("create aggregation fixture directory")?;
    let root = Utf8Path::from_path(guard.path())
        .ok_or_else(|| anyhow!("temporary fixture path is not UTF-8"))?
        .to_owned();
    let dir = Dir::open_ambient_dir(&root, ambient_authority())
        .context("open aggregation fixture directory")?;
    Ok(Fixture {
        _temp_dir: guard,
        root,
        dir,
    })
}

/// Start a `Fixture` root's environment with the XDG bases pinned to it.
///
/// Wrapping the entry point keeps the remaining `MapEnv` chain per test while
/// guaranteeing no case silently falls back to `/etc/xdg`.
fn fixture_env(root: &Utf8Path) -> MapEnv {
    with_host_overrides(MapEnv::new(), root.as_std_path())
}

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

/// CLI, file, and environment faults are gathered into one aggregate error.
#[rstest]
fn aggregates_cli_file_env_errors(fixture: Result<Fixture>) -> Result<()> {
    let staged = fixture?;
    let fixture_root = &staged.root;
    let config_path = fixture_root.join(".config.toml");
    staged
        .dir
        .write(".config.toml", b"port = ")
        .context("write invalid configuration fixture")?;

    let source = Arc::new(
        fixture_env(fixture_root)
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

/// An invalid higher-ranked candidate is hidden when a fallback file loads.
#[rstest]
fn discovery_errors_hidden_when_fallback_succeeds(fixture: Result<Fixture>) -> Result<()> {
    let staged = fixture?;
    let fixture_root = &staged.root;
    let invalid_path = fixture_root.join("invalid.toml");
    staged
        .dir
        .write("invalid.toml", b"port = ???")
        .context("write invalid selector fixture")?;
    staged
        .dir
        .write(".agg.toml", b"port = 7000")
        .context("write fallback configuration fixture")?;

    let source = Arc::new(
        fixture_env(fixture_root)
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

/// A required `--config-path` that is absent fails despite a valid fallback.
#[rstest]
fn required_path_errors_surface_even_with_fallback(fixture: Result<Fixture>) -> Result<()> {
    let staged = fixture?;
    let fixture_root = &staged.root;
    let missing_path = fixture_root.join("missing.toml");
    staged
        .dir
        .write(".agg.toml", b"port = 7000")
        .context("write fallback configuration fixture")?;

    let source =
        Arc::new(fixture_env(fixture_root).with_var("XDG_CONFIG_HOME", fixture_root.as_os_str()));
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

/// A discovery error surfaces when every candidate, including fallbacks, fails.
#[rstest]
fn discovery_errors_surface_when_all_candidates_fail(fixture: Result<Fixture>) -> Result<()> {
    let staged = fixture?;
    let fixture_root = &staged.root;
    let invalid_path = fixture_root.join("invalid.toml");
    staged
        .dir
        .write("invalid.toml", b"port = ???")
        .context("write invalid selector fixture")?;

    let source = Arc::new(
        fixture_env(fixture_root)
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
