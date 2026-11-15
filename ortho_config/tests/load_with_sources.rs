//! Complete derived configuration loading from injected environment sources.
//!
//! This test keeps discovery and merge data in one `MapEnv`. It must not mutate
//! the process environment: that would hide a failure to thread either source
//! through the derive-generated loading path.

use anyhow::{Context as _, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, OrthoConfig, ProfileSource, SharedEnvSource, SharedScanEnvSource};
use rstest::{fixture, rstest};
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

/// Opts into profile support so the selector lookup path is exercised.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "PROFILED_CONFIG_", profiles)]
struct ProfiledConfig {
    #[ortho_config(default = 1)]
    retries: u8,
}

/// Write a selector fixture through a capability handle.
fn write_selector_fixture(dir: &Path, contents: &[u8]) -> Result<PathBuf> {
    let cap = Dir::open_ambient_dir(dir, ambient_authority())
        .context("open source-aware loading fixture directory")?;
    cap.write("selected.toml", contents)
        .context("write source-aware loading fixture")?;
    Ok(dir.join("selected.toml"))
}

/// Create the isolated root used by source-aware loading scenarios.
#[fixture]
fn loading_fixture_dir() -> Result<tempfile::TempDir> {
    tempfile::tempdir().context("create source-aware loading fixture")
}

/// Write a config file defining profile `ci`, returning its path.
///
/// The write goes through a `cap_std::fs::Dir` handle rather than `std::fs`,
/// as the repository's lint suite requires.
fn write_profiled_fixture(dir: &Path) -> Result<PathBuf> {
    let cap = Dir::open_ambient_dir(dir, ambient_authority())
        .context("open profiled loading fixture directory")?;
    cap.write("profiled.toml", b"retries = 3\n[profile.ci]\nretries = 7\n")
        .context("write profiled loading fixture")?;
    Ok(dir.join("profiled.toml"))
}

/// A selector present only in the injected map must steer the selection.
///
/// The map also supplies the config-file selector, so the profile table is
/// reachable only if discovery, selection, and extraction all read through the
/// injected source. If the generated loader fell back to `std::env::var` for
/// the selector, the injected `PROFILED_CONFIG_PROFILE` would be ignored and
/// the profile value would never reach the merge.
#[test]
fn profiled_loading_reads_the_selector_from_the_injected_source() -> Result<()> {
    let fixture_dir = tempfile::tempdir().context("create profiled loading fixture")?;
    let config_path = write_profiled_fixture(fixture_dir.path())?;
    // Prove the test cannot pass by reading the process environment.
    ensure!(
        std::env::var_os("PROFILED_CONFIG_PROFILE").is_none(),
        "the process must not already define the selector this test injects"
    );
    let source = Arc::new(
        MapEnv::new()
            .with_var("PROFILED_CONFIG_CONFIG_PATH", &config_path)
            .with_var("PROFILED_CONFIG_PROFILE", "ci"),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;

    let config = ProfiledConfig::load_from_iter_with_sources(["profiled-config"], discovery, merge)
        .map_err(|error| anyhow::anyhow!(error))
        .context("load a profile-selected configuration from an injected source")?;

    // Three distinguishable outcomes make this assertion decisive: the struct
    // default is 1, the discovered file sets 3, and only profile `ci` sets 7.
    // A selector that read the process instead would return 3 here.
    ensure!(
        config.retries == 7,
        "expected the injected selector to select profile `ci`, got {}",
        config.retries
    );
    Ok(())
}

/// The reporting entry point must honour the injected selector as well, and
/// must report that selection rather than discarding it.
///
/// This is the hermetic counterpart of the jail-backed BDD scenario: it needs
/// no process-environment mutation, so it keeps running concurrently with the
/// rest of this file. The reported selection is what distinguishes this case
/// from `profiled_loading_reads_the_selector_from_the_injected_source`, which
/// observes only the merged value.
#[test]
fn source_aware_profile_load_reports_the_injected_selection() -> Result<()> {
    let fixture_dir = tempfile::tempdir().context("create profiled loading fixture")?;
    let config_path = write_profiled_fixture(fixture_dir.path())?;
    ensure!(
        std::env::var_os("PROFILED_CONFIG_PROFILE").is_none(),
        "the process must not already define the selector this test injects"
    );
    let source = Arc::new(
        MapEnv::new()
            .with_var("PROFILED_CONFIG_CONFIG_PATH", &config_path)
            .with_var("PROFILED_CONFIG_PROFILE", "ci"),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;

    let outcome = ProfiledConfig::load_with_profile_from_iter_with_sources(
        ["profiled-config"],
        discovery,
        merge,
    )
    .map_err(|error| anyhow::anyhow!(error))
    .context("load a profile-selected outcome from an injected source")?;

    let [selection] = outcome.selection() else {
        return Err(anyhow::anyhow!(
            "expected exactly one selected profile, got {:?}",
            outcome.selection()
        ));
    };
    ensure!(
        selection.name.as_str() == "ci",
        "expected the injected selector to report profile `ci`, got {:?}",
        selection.name
    );
    ensure!(
        selection.source == ProfileSource::Environment,
        "the injected selector is an environment source, got {:?}",
        selection.source
    );
    ensure!(
        outcome.config().retries == 7,
        "expected profile `ci` to supply 7, got {}",
        outcome.config().retries
    );
    Ok(())
}

/// A non-UTF-8 injected selector is reported rather than silently ignored.
///
/// The injected selector arrives as an `OsString`, so the loader can read it
/// successfully and still fail to transcode it. Treating that as "no selector"
/// would return the base configuration while the caller believed a profile had
/// been applied, so the load must fail instead.
///
/// Gated to Unix because building a non-UTF-8 `OsString` from raw bytes needs
/// `std::os::unix::ffi::OsStringExt`; there is no portable constructor.
#[cfg(unix)]
#[test]
fn a_non_utf8_injected_selector_is_reported_not_ignored() -> Result<()> {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt as _};

    let fixture_dir = tempfile::tempdir().context("create profiled loading fixture")?;
    let config_path = write_profiled_fixture(fixture_dir.path())?;
    // The fixture defines profile `ci`, so a loader that dropped the selector
    // would still succeed with the base value of 3. Pushing 0xFF makes the
    // value non-representable however the preceding bytes happen to decode.
    let mut raw = b"ci".to_vec();
    raw.push(0xFF);
    let selector = OsString::from_vec(raw);

    let source = Arc::new(
        MapEnv::new()
            .with_var("PROFILED_CONFIG_CONFIG_PATH", &config_path)
            .with_var("PROFILED_CONFIG_PROFILE", &selector),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;

    let error = ProfiledConfig::load_with_profile_from_iter_with_sources(
        ["profiled-config"],
        discovery,
        merge,
    )
    .map(|outcome| outcome.config().retries)
    .expect_err("a non-Unicode selector must be reported, not ignored");
    let message = error.to_string();
    ensure!(
        message.contains("not valid Unicode"),
        "the diagnostic must name the encoding fault, got: {message}"
    );
    Ok(())
}

/// One map drives both the selector lookup and the complete merge layer.
///
/// The process-backed selector path is covered by the `profiles.feature` BDD
/// suite, which owns the process-environment scenarios; this file stays free of
/// process mutation so its cases keep running concurrently.
#[rstest]
fn derived_loading_uses_one_map_for_both_environment_capabilities(
    loading_fixture_dir: Result<tempfile::TempDir>,
) -> Result<()> {
    let fixture_dir = loading_fixture_dir?;
    let selector_path = write_selector_fixture(fixture_dir.path(), b"from_file = \"selected\"\n")?;
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
#[rstest]
fn unprefixed_derived_loading_replays_generated_key_transforms(
    loading_fixture_dir: Result<tempfile::TempDir>,
) -> Result<()> {
    let fixture_dir = loading_fixture_dir?;
    // An explicit selector avoids searching ambient CWD candidates, though the
    // discovery builder still resolves the current directory.
    let selector_path = write_selector_fixture(fixture_dir.path(), b"# selector fixture\n")?;
    let source = Arc::new(
        MapEnv::new()
            .with_var("CONFIG_PATH", &selector_path)
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

/// A malformed optional candidate must not suppress an unknown-profile error.
///
/// Candidate order is what makes this case decisive: the selector is pushed
/// before the XDG bases, so the malformed selector is walked first and its
/// error recorded, and the later XDG file supplies the chain that survives.
/// `discovery_failed` answers "were the profile tables inspected", so it must
/// stay clear here — the surviving chain *was* read, and it defines no `ci`
/// table. Widening the flag to "did any candidate fail" would let this
/// unrelated parse complaint hide the genuine selection error, and the load
/// would silently return the base value of 3 while `selection()` still named
/// `ci`.
#[test]
fn a_malformed_optional_candidate_still_reports_an_unknown_profile() -> Result<()> {
    let fixture_dir = tempfile::tempdir().context("create profiled loading fixture")?;
    // Written through a capability handle, as the repository's filesystem
    // lints require. The selector names this file directly and is itself
    // optional, so its parse failure is recorded without stopping the walk.
    let cap = Dir::open_ambient_dir(fixture_dir.path(), ambient_authority())
        .context("open profiled loading fixture directory")?;
    cap.write("broken.toml", b"this is not toml = = =\n")
        .context("write malformed selector candidate")?;
    let broken_path = fixture_dir.path().join("broken.toml");
    // The fallback the walk reaches next, at the name discovery derives from
    // the struct's `PROFILED_CONFIG_` prefix: `<base>/<app_name>/config.toml`.
    // It parses, so it becomes the chain, and it defines no profile at all.
    let xdg_dir = fixture_dir.path().join("xdg");
    cap.create_dir("xdg").context("create xdg fixture dir")?;
    cap.create_dir("xdg/profiled_config")
        .context("create profiled config dir")?;
    cap.write("xdg/profiled_config/config.toml", b"retries = 3\n")
        .context("write profile-less fallback candidate")?;

    let source = Arc::new(
        MapEnv::new()
            .with_var("PROFILED_CONFIG_CONFIG_PATH", &broken_path)
            .with_var("PROFILED_CONFIG_PROFILE", "ci")
            .with_var("XDG_CONFIG_HOME", &xdg_dir),
    );
    let discovery: SharedEnvSource = source.clone();
    let merge: SharedScanEnvSource = source;

    let error = ProfiledConfig::load_from_iter_with_sources(["profiled-config"], discovery, merge)
        .map(|config| config.retries)
        .expect_err("the inspected chain defines no `ci`, so the selection is unknown");
    let message = error.to_string();
    ensure!(
        message.contains("ci"),
        "the error must name the unknown selection, got: {message}"
    );
    ensure!(
        message.contains("no profiles were found"),
        "the chain was read and holds no profile tables, got: {message}"
    );
    Ok(())
}
