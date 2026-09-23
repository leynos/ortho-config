//! Unit tests for injected subcommand configuration path resolution.

#[cfg(any(unix, target_os = "redox"))]
use super::*;
#[cfg(any(unix, target_os = "redox"))]
use crate::{EnvSource, MapEnv};
#[cfg(any(unix, target_os = "redox"))]
use anyhow::{Context, Result, ensure};
#[cfg(any(unix, target_os = "redox"))]
use std::ffi::OsString;
#[cfg(any(unix, target_os = "redox"))]
use std::fs;
#[cfg(any(unix, target_os = "redox"))]
use std::path::PathBuf;
#[cfg(any(unix, target_os = "redox"))]
use tempfile::TempDir;

#[cfg(not(any(unix, target_os = "redox")))]
use super::*;
#[cfg(not(any(unix, target_os = "redox")))]
use crate::{EnvSource, MapEnv};
#[cfg(not(any(unix, target_os = "redox")))]
use anyhow::{Result, ensure};
#[cfg(not(any(unix, target_os = "redox")))]
use std::ffi::OsString;
#[cfg(not(any(unix, target_os = "redox")))]
use std::path::PathBuf;

#[cfg(any(unix, target_os = "redox"))]
#[derive(Debug)]
struct FallbackEnv {
    vars: MapEnv,
    home: PathBuf,
}

#[cfg(any(unix, target_os = "redox"))]
impl EnvSource for FallbackEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.vars.get(key)
    }
    fn home_fallback(&self) -> Option<PathBuf> {
        Some(self.home.clone())
    }
}

#[cfg(not(any(unix, target_os = "redox")))]
#[derive(Debug)]
struct PlatformFallbackEnv {
    vars: MapEnv,
    config_dir: Option<PathBuf>,
}

#[cfg(not(any(unix, target_os = "redox")))]
impl EnvSource for PlatformFallbackEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.vars.get(key)
    }

    fn config_dir_fallback(&self) -> Option<PathBuf> {
        self.config_dir.clone()
    }
}

#[cfg(not(any(unix, target_os = "redox")))]
fn extension_paths(dir: &std::path::Path, stem: &str) -> Vec<PathBuf> {
    EXT_GROUPS
        .iter()
        .flat_map(|group| *group)
        .map(|extension| dir.join(format!("{stem}.{extension}")))
        .collect()
}

/// An injected platform root follows home candidates and precedes the local base.
#[cfg(not(any(unix, target_os = "redox")))]
#[test]
fn injected_platform_config_root_preserves_candidate_order() -> Result<()> {
    let home = PathBuf::from("C:/injected/home");
    let config_root = PathBuf::from("C:/injected/config");
    let base = PathBuf::from("C:/injected/base");
    let source = PlatformFallbackEnv {
        vars: MapEnv::new().with_var("HOME", &home),
        config_dir: Some(config_root.clone()),
    };
    let prefix = Prefix::new("app");
    let paths = candidate_paths_at(&prefix, &base, &source);
    let mut expected = extension_paths(&home, ".app");
    expected.extend(extension_paths(&config_root.join("app"), "config"));
    expected.extend(extension_paths(&base, ".app"));
    ensure!(
        paths == expected,
        "platform candidate order differs: {paths:?}"
    );
    Ok(())
}

/// A closed map source never reads the host platform configuration directory.
#[cfg(not(any(unix, target_os = "redox")))]
#[test]
fn map_env_omits_ambient_platform_config_root() {
    let base = PathBuf::from("C:/injected/base");
    let paths = candidate_paths_at(&Prefix::new("app"), &base, &MapEnv::new());
    assert_eq!(paths, extension_paths(&base, ".app"));
}

#[cfg(any(unix, target_os = "redox"))]
#[test]
fn injected_candidates_preserve_home_xdg_and_base_order() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let home = root.path().join("home");
    let xdg = root.path().join("xdg");
    let base = root.path().join("base");
    fs::create_dir_all(xdg.join("app")).context("create XDG directory")?;
    fs::create_dir_all(&base).context("create local base")?;
    fs::write(xdg.join("app/config.toml"), "").context("write XDG config")?;
    let paths = candidate_paths_at(
        &Prefix::new("app"),
        &base,
        &MapEnv::new()
            .with_var("HOME", &home)
            .with_var("XDG_CONFIG_HOME", &xdg),
    );
    ensure!(
        paths.first() == Some(&home.join(".app.toml")),
        "home must be first"
    );
    let extension_count: usize = EXT_GROUPS.iter().map(|group| group.len()).sum();
    let home_candidates: Vec<PathBuf> = EXT_GROUPS
        .iter()
        .flat_map(|group| *group)
        .map(|extension| home.join(format!(".app.{extension}")))
        .collect();
    let base_candidates: Vec<PathBuf> = EXT_GROUPS
        .iter()
        .flat_map(|group| *group)
        .map(|extension| base.join(format!(".app.{extension}")))
        .collect();
    let xdg_index = paths
        .iter()
        .position(|path| path == &xdg.join("app/config.toml"))
        .ok_or_else(|| anyhow::anyhow!("XDG candidate missing"))?;
    ensure!(
        paths.get(..extension_count) == Some(home_candidates.as_slice()),
        "home extension order differs"
    );
    ensure!(
        xdg_index == extension_count,
        "XDG must follow home candidates"
    );
    let base_start = paths
        .len()
        .checked_sub(extension_count)
        .ok_or_else(|| anyhow::anyhow!("base extension candidates missing"))?;
    ensure!(xdg_index < base_start, "XDG must precede base candidates");
    ensure!(
        paths.get(xdg_index..base_start) == Some([xdg.join("app/config.toml")].as_slice()),
        "XDG group differs"
    );
    ensure!(
        paths.get(base_start..) == Some(base_candidates.as_slice()),
        "base extension order differs"
    );
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[test]
fn relative_xdg_home_uses_injected_fallback() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let home = root.path().join("home");
    fs::create_dir_all(home.join(".config/app")).context("create fallback")?;
    fs::write(home.join(".config/app/config.toml"), "").context("write config")?;
    let env = FallbackEnv {
        vars: MapEnv::new().with_var("XDG_CONFIG_HOME", "relative"),
        home: home.clone(),
    };
    let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &env);
    ensure!(
        paths.contains(&home.join(".config/app/config.toml")),
        "fallback config missing"
    );
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[rstest::rstest]
#[case::unset(None)]
#[case::relative(Some("relative"))]
fn xdg_home_uses_injected_home_when_value_is_unusable(
    #[case] xdg_home: Option<&str>,
) -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let home = root.path().join("home");
    let config = home.join(".config/app/config.toml");
    fs::create_dir_all(config.parent().context("find config parent")?)
        .context("create injected home config directory")?;
    fs::write(&config, "").context("write injected home config")?;
    let mut source = MapEnv::new().with_var("HOME", &home);
    if let Some(value) = xdg_home {
        source = source.with_var("XDG_CONFIG_HOME", value);
    }

    let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &source);
    ensure!(paths.contains(&config), "injected HOME config missing");
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[test]
fn closed_map_omits_host_xdg_home() {
    let bases = source_xdg_bases(&Prefix::new("app"), &MapEnv::new());
    assert_eq!(bases, vec![PathBuf::from("/etc/xdg/app")]);
}

#[cfg(any(unix, target_os = "redox"))]
#[rstest::rstest]
#[case::empty("")]
#[case::relative("relative")]
fn unusable_xdg_dirs_use_platform_default(#[case] dirs: &str) {
    let bases = source_xdg_bases(
        &Prefix::new("app"),
        &MapEnv::new().with_var("XDG_CONFIG_DIRS", dirs),
    );
    assert_eq!(bases, vec![PathBuf::from("/etc/xdg/app")]);
}

#[cfg(any(unix, target_os = "redox"))]
#[test]
fn xdg_dirs_keep_absolute_entries_in_order() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let first = root.path().join("first");
    let second = root.path().join("second");
    let dirs = std::env::join_paths([first.as_path(), Path::new("relative"), second.as_path()])
        .context("join XDG configuration directories")?;
    let bases = source_xdg_bases(
        &Prefix::new("app"),
        &MapEnv::new().with_var("XDG_CONFIG_DIRS", dirs),
    );
    ensure!(
        bases == [first.join("app"), second.join("app")],
        "absolute XDG directories changed order: {bases:?}"
    );
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[test]
fn xdg_extension_search_uses_first_existing_path() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let home = root.path().join("home");
    let dirs = root.path().join("dirs");
    fs::create_dir_all(home.join("app")).context("create home")?;
    fs::create_dir_all(dirs.join("app")).context("create dirs")?;
    fs::write(home.join("app/config.toml"), "").context("write toml")?;
    #[cfg(feature = "json5")]
    fs::write(dirs.join("app/config.json"), "").context("write json")?;
    let env = FallbackEnv {
        vars: MapEnv::new()
            .with_var("XDG_CONFIG_HOME", &home)
            .with_var("XDG_CONFIG_DIRS", &dirs),
        home,
    };
    let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &env);
    ensure!(
        paths.first() == Some(&env.home.join("app/config.toml")),
        "toml order differs"
    );
    #[cfg(feature = "json5")]
    ensure!(
        paths.get(1) == Some(&dirs.join("app/config.json")),
        "json first-existing differs"
    );
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[test]
fn xdg_search_keeps_metadata_existence_contract() -> Result<()> {
    let root = TempDir::new().context("create root")?;
    let config = root.path().join("app/config.toml");
    fs::create_dir_all(&config).context("create directory at config path")?;
    let source = MapEnv::new().with_var("XDG_CONFIG_HOME", root.path());
    let paths = candidate_paths_at(&Prefix::new("app"), root.path(), &source);
    ensure!(
        paths.contains(&config),
        "XDG 3 treats any path with metadata as an existing candidate"
    );
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
const XDG_ORACLE_ARG: &str = "--ortho-config-xdg-oracle";
#[cfg(any(unix, target_os = "redox"))]
const XDG_ORACLE_RECORD: &str = "ortho-config-xdg3:";

#[cfg(any(unix, target_os = "redox"))]
#[test]
fn xdg_3_oracle_child() -> Result<()> {
    use std::io::Write;
    use xdg::BaseDirectories;

    if !std::env::args().any(|argument| argument == XDG_ORACLE_ARG) {
        return Ok(());
    }
    let directories = BaseDirectories::with_prefix("oracle");
    let mut stdout = std::io::stdout().lock();
    for group in EXT_GROUPS {
        for extension in *group {
            if let Some(path) = directories.find_config_file(format!("config.{extension}")) {
                writeln!(stdout, "{XDG_ORACLE_RECORD}{}", path.display())
                    .context("write XDG oracle record")?;
            }
        }
    }
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[test]
fn injected_xdg_resolution_matches_xdg_3_child_oracle() -> Result<()> {
    use std::process::Command;

    let root = TempDir::new().context("create oracle root")?;
    let home = root.path().join("home");
    fs::create_dir_all(home.join(".config/oracle")).context("create fallback XDG directory")?;
    fs::write(home.join(".config/oracle/config.toml"), "").context("write fallback config")?;

    let output = Command::new(std::env::current_exe().context("locate test binary")?)
        .args([
            "--exact",
            "subcommand::paths::tests::xdg_3_oracle_child",
            "--nocapture",
            "--",
            XDG_ORACLE_ARG,
        ])
        .env_clear()
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", "relative")
        .current_dir(root.path())
        .output()
        .context("run XDG 3 oracle child")?;
    ensure!(
        output.status.success(),
        "XDG 3 oracle child failed: {output:?}"
    );
    let oracle = String::from_utf8(output.stdout).context("decode oracle output")?;
    let oracle_paths: Vec<PathBuf> = oracle
        .lines()
        .filter_map(|line| line.strip_prefix(XDG_ORACLE_RECORD))
        .map(PathBuf::from)
        .collect();
    ensure!(
        !oracle_paths.is_empty(),
        "XDG 3 oracle emitted no framed paths"
    );
    let source = FallbackEnv {
        vars: MapEnv::new().with_var("XDG_CONFIG_HOME", "relative"),
        home: home.clone(),
    };
    let injected: Vec<PathBuf> = candidate_paths_at(&Prefix::new("oracle"), root.path(), &source)
        .into_iter()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("config."))
        })
        .collect();
    ensure!(
        injected == oracle_paths,
        "injected XDG paths differ from XDG 3 oracle"
    );
    Ok(())
}
