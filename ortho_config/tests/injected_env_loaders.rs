//! The generated `_with_env` loaders and the paths an injected source names.
//!
//! One `MapEnv` backs both the discovery and the merge layer, and pins the
//! working directory. These cases cover the composition and profile entry
//! points, and the rule that a *relative* path the source names resolves
//! against the source's working directory rather than the process's.

use anyhow::{Context as _, Result, anyhow, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{MapEnv, MergeLayer, MergeProvenance, OrthoConfig, OrthoError};
use rstest::rstest;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

/// Plain configuration found through its project dotfile.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "CWDENV_")]
struct PlainConfig {
    #[ortho_config(default = 1)]
    port: u16,
}

/// Profile-enabled configuration, selected by `CWDENV_PROFILE`.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "CWDENVP_", profiles)]
struct ProfiledConfig {
    #[ortho_config(default = 1)]
    retries: u8,
}

/// Write `body` to `file` beneath `dir`, creating parent directories.
fn write_file(dir: &Path, file: &str, body: &str) -> Result<()> {
    let directory =
        Dir::open_ambient_dir(dir, ambient_authority()).context("open the fixture directory")?;
    if let Some(parent) = Path::new(file)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        directory
            .create_dir_all(parent)
            .context("create the fixture subdirectory")?;
    }
    directory.write(file, body).context("write the fixture")
}

/// A closed environment whose working directory is `dir`.
fn env_in(dir: &Path) -> MapEnv {
    MapEnv::new()
        .with_current_dir(dir)
        .with_var("XDG_CONFIG_DIRS", dir.join("xdg-dirs-empty"))
}

/// `compose_layers_from_iter_with_env` reads the pinned directory's file and
/// the injected environment, and stacks them in precedence order.
#[test]
fn compose_with_env_stacks_the_file_and_environment_layers() -> Result<()> {
    let dir = tempfile::tempdir().context("create the fixture directory")?;
    write_file(dir.path(), ".cwdenv.toml", "port = 2000\n")?;
    let env = Arc::new(env_in(dir.path()).with_var("CWDENV_PORT", "3000"));

    let composition = PlainConfig::compose_layers_from_iter_with_env(["prog"], env);
    let (layers, errors) = composition.into_parts();

    ensure!(errors.is_empty(), "composition errors: {errors:?}");
    let provenances: Vec<MergeProvenance> = layers.iter().map(MergeLayer::provenance).collect();
    ensure!(
        provenances
            == [
                MergeProvenance::Defaults,
                MergeProvenance::File,
                MergeProvenance::Environment
            ],
        "unexpected provenance stack: {provenances:?}"
    );
    let merged = PlainConfig::merge_from_layers(layers).map_err(|err| anyhow!(err))?;
    ensure!(
        merged.port == 3000,
        "the environment must win over the file, got {}",
        merged.port
    );
    Ok(())
}

/// The same composition without the environment value shows the file's own
/// value, and without any file shows the default.
#[rstest]
#[case::file_value(Some("port = 2000\n"), 2000)]
#[case::absent_file_default(None, 1)]
fn compose_with_env_reads_the_file_or_falls_back_to_the_default(
    #[case] file: Option<&str>,
    #[case] expected: u16,
) -> Result<()> {
    let dir = tempfile::tempdir().context("create the fixture directory")?;
    if let Some(body) = file {
        write_file(dir.path(), ".cwdenv.toml", body)?;
    }

    let (layers, errors) =
        PlainConfig::compose_layers_from_iter_with_env(["prog"], Arc::new(env_in(dir.path())))
            .into_parts();

    ensure!(errors.is_empty(), "composition errors: {errors:?}");
    let has_file_layer = layers
        .iter()
        .any(|layer| layer.provenance() == MergeProvenance::File);
    ensure!(
        has_file_layer == file.is_some(),
        "the file layer must be present exactly when a file exists"
    );
    let merged = PlainConfig::merge_from_layers(layers).map_err(|err| anyhow!(err))?;
    ensure!(merged.port == expected, "got {}", merged.port);
    Ok(())
}

/// An unknown profile name fails the load with a structured `UnknownProfile`.
#[test]
fn profile_load_with_env_reports_an_unknown_profile() -> Result<()> {
    let dir = tempfile::tempdir().context("create the fixture directory")?;
    write_file(
        dir.path(),
        ".cwdenvp.toml",
        "retries = 3\n[profile.ci]\nretries = 7\n",
    )?;

    let outcome = ProfiledConfig::load_with_profile_from_iter_with_env(
        ["prog"],
        Arc::new(env_in(dir.path()).with_var("CWDENVP_PROFILE", "missing")),
    );

    let error = outcome
        .err()
        .ok_or_else(|| anyhow!("an unknown profile must fail the load"))?;
    ensure!(
        matches!(
            error.as_ref(),
            OrthoError::UnknownProfile { selected, .. } if selected == "missing"
        ),
        "expected UnknownProfile naming `missing`, got {error:?}"
    );
    Ok(())
}

/// `load_with_profile_from_iter_with_env` selects a profile from the injected
/// environment and applies it over the pinned directory's file.
#[test]
fn profile_load_with_env_selects_the_injected_profile() -> Result<()> {
    let dir = tempfile::tempdir().context("create the fixture directory")?;
    write_file(
        dir.path(),
        ".cwdenvp.toml",
        "retries = 3\n[profile.ci]\nretries = 7\n",
    )?;

    let plain = ProfiledConfig::load_with_profile_from_iter_with_env(
        ["prog"],
        Arc::new(env_in(dir.path())),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(plain.config().retries == 3, "base file value expected");

    let ci = ProfiledConfig::load_with_profile_from_iter_with_env(
        ["prog"],
        Arc::new(env_in(dir.path()).with_var("CWDENVP_PROFILE", "ci")),
    )
    .map_err(|err| anyhow!(err))?;
    ensure!(
        ci.config().retries == 7,
        "profile `ci` must apply, got {}",
        ci.config().retries
    );
    let selection = ci.selection();
    let [selected] = selection else {
        return Err(anyhow!("expected one selected profile, got {selection:?}"));
    };
    ensure!(
        selected.name.as_str() == "ci",
        "expected profile `ci` to be selected, got {selected:?}"
    );
    Ok(())
}

/// A relative config-path selector resolves against the pinned directory, not
/// the process's.
#[test]
fn a_relative_selector_resolves_against_the_pinned_directory() -> Result<()> {
    let dir = tempfile::tempdir().context("create the fixture directory")?;
    write_file(dir.path(), "chosen.toml", "port = 4242\n")?;
    let env = env_in(dir.path()).with_var("CWDENV_CONFIG_PATH", "chosen.toml");

    let config = PlainConfig::load_from_iter_with_env(["prog"], Arc::new(env))
        .map_err(|err| anyhow!(err))?;

    ensure!(config.port == 4242, "got {}", config.port);
    Ok(())
}

/// A relative `XDG_CONFIG_HOME` resolves against the pinned directory.
#[test]
fn a_relative_xdg_config_home_resolves_against_the_pinned_directory() -> Result<()> {
    let dir = tempfile::tempdir().context("create the fixture directory")?;
    write_file(dir.path(), "xdg/cwdenv/config.toml", "port = 5151\n")?;
    let env = env_in(dir.path()).with_var("XDG_CONFIG_HOME", "xdg");

    let config = PlainConfig::load_from_iter_with_env(["prog"], Arc::new(env))
        .map_err(|err| anyhow!(err))?;

    ensure!(config.port == 5151, "got {}", config.port);
    Ok(())
}

/// A relative selector is dropped, not read from the process, when the source
/// cannot report a working directory.
#[test]
fn a_relative_selector_is_dropped_when_the_directory_is_unknown() -> Result<()> {
    #[derive(Debug)]
    struct Unlocated(MapEnv);
    impl ortho_config::EnvSource for Unlocated {
        fn get(&self, key: &str) -> Option<std::ffi::OsString> {
            self.0.get(key)
        }
        fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
            Err(std::io::Error::other("deleted"))
        }
    }
    impl ortho_config::ScanEnvSource for Unlocated {
        fn scan(&self) -> Vec<(std::ffi::OsString, std::ffi::OsString)> {
            self.0.scan()
        }
    }

    let env = Unlocated(MapEnv::new().with_var("CWDENV_CONFIG_PATH", "chosen.toml"));
    let config = PlainConfig::load_from_iter_with_env(["prog"], Arc::new(env))
        .map_err(|err| anyhow!(err))?;

    ensure!(
        config.port == 1,
        "expected the default, got {}",
        config.port
    );
    Ok(())
}
