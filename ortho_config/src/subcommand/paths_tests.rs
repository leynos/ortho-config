//! Unit tests for subcommand configuration path resolution.

#[cfg(any(unix, target_os = "redox"))]
use super::*;
#[cfg(any(unix, target_os = "redox"))]
use anyhow::{Context, Result, anyhow, ensure};
#[cfg(any(unix, target_os = "redox"))]
use rstest::rstest;
#[cfg(any(unix, target_os = "redox"))]
use std::fs;
#[cfg(any(unix, target_os = "redox"))]
use std::path::Path;
#[cfg(any(unix, target_os = "redox"))]
use tempfile::TempDir;

#[cfg(any(unix, target_os = "redox"))]
use test_helpers::env::{self as test_env, EnvVarGuard};

/// Creates a temporary XDG config directory and sets `XDG_CONFIG_HOME` for the test.
#[cfg(any(unix, target_os = "redox"))]
fn init_xdg_home() -> Result<(TempDir, EnvVarGuard)> {
    let dir = TempDir::new().context("create XDG config temp directory")?;
    let guard = test_env::set_var("XDG_CONFIG_HOME", dir.path());
    Ok((dir, guard))
}

#[cfg(any(unix, target_os = "redox"))]
#[rstest]
#[case(&["toml"], &["config.toml"])]
#[cfg(feature = "json5")]
#[case(&["json", "json5"], &["config.json", "config.json5"])]
#[cfg(feature = "yaml")]
#[case(&["yaml", "yml"], &["config.yaml", "config.yml"])]
fn push_xdg_candidates_finds_files(#[case] exts: &[&str], #[case] files: &[&str]) -> Result<()> {
    let _env_lock = test_env::lock();
    let (xdg_tempdir, _guard) = init_xdg_home()?;
    let dir_path = xdg_tempdir.path();
    // TempDir is empty on creation; no cleanup required.

    for file in files {
        fs::write(dir_path.join(file), "").with_context(|| format!("create test file {file}"))?;
    }

    let dirs = BaseDirectories::new();
    let mut paths = Vec::new();
    push_xdg_candidates(&dirs, exts, &mut paths);

    ensure!(
        paths.len() == files.len(),
        "expected {:?} to locate {} files, found {}",
        exts,
        files.len(),
        paths.len()
    );
    for (p, f) in paths.iter().zip(files.iter()) {
        ensure!(
            p == &dir_path.join(f),
            "unexpected path {p:?} for expected file {f}"
        );
    }
    Ok(())
}

#[cfg(any(unix, target_os = "redox"))]
#[rstest]
#[case("")]
#[case("myapp")]
fn candidate_paths_ordering(#[case] prefix_raw: &str) -> Result<()> {
    let _env_lock = test_env::lock();
    let home = TempDir::new().context("create home directory")?;
    let home_guard = test_env::set_var("HOME", home.path());

    let (base_dir, _guard) = init_xdg_home()?;
    let base = base_dir.path();
    let xdg_cfg_dir = if prefix_raw.is_empty() {
        base.to_path_buf()
    } else {
        let d = base.join(prefix_raw);
        fs::create_dir_all(&d).context("create prefixed XDG directory")?;
        d
    };

    // Populate every XDG candidate so `find_config_file` resolves each one
    // inside the fixture rather than falling back to host `XDG_CONFIG_DIRS`.
    fs::write(xdg_cfg_dir.join("config.toml"), "").context("write config.toml")?;
    #[cfg(feature = "json5")]
    {
        fs::write(xdg_cfg_dir.join("config.json"), "").context("write config.json")?;
        fs::write(xdg_cfg_dir.join("config.json5"), "").context("write config.json5")?;
    }
    #[cfg(feature = "yaml")]
    {
        fs::write(xdg_cfg_dir.join("config.yaml"), "").context("write config.yaml")?;
        fs::write(xdg_cfg_dir.join("config.yml"), "").context("write config.yml")?;
    }

    let prefix = Prefix::new(prefix_raw);
    let paths = candidate_paths(&prefix);

    let dotted_prefix = dotted(&prefix);

    let mut expected_files = Vec::new();
    for ext in EXT_GROUPS.iter().flat_map(|g| *g) {
        expected_files.push(format!("{dotted_prefix}.{ext}"));
    }
    expected_files.push("config.toml".to_owned());
    #[cfg(feature = "json5")]
    {
        expected_files.push("config.json".to_owned());
        expected_files.push("config.json5".to_owned());
    }
    #[cfg(feature = "yaml")]
    {
        expected_files.push("config.yaml".to_owned());
        expected_files.push("config.yml".to_owned());
    }
    for ext in EXT_GROUPS.iter().flat_map(|g| *g) {
        expected_files.push(format!("{dotted_prefix}.{ext}"));
    }

    let files: Vec<String> = paths
        .iter()
        .map(|p| {
            p.file_name()
                .ok_or_else(|| anyhow!("candidate path missing file name"))
                .map(|name| name.to_string_lossy().into_owned())
        })
        .collect::<Result<_>>()?;
    ensure!(
        files == expected_files,
        "unexpected candidate ordering: {files:?}"
    );

    let group_len: usize = EXT_GROUPS.iter().map(|g| g.len()).sum();

    ensure!(
        paths
            .iter()
            .take(group_len)
            .all(|p| p.parent() == Some(home.path())),
        "home candidates must reside in the HOME fixture directory"
    );

    let xdg_slice = paths
        .get(group_len..paths.len().saturating_sub(group_len))
        .unwrap_or(&[]);
    ensure!(
        !xdg_slice.is_empty(),
        "platform candidates must include the XDG fixture files"
    );
    ensure!(
        xdg_slice
            .iter()
            .all(|p| p.parent() == Some(xdg_cfg_dir.as_path())),
        "platform candidates must reside in the XDG fixture directory"
    );

    let local_slice = paths
        .get(paths.len().saturating_sub(group_len)..)
        .unwrap_or(&[]);
    ensure!(
        local_slice.len() == group_len,
        "local candidate count must equal EXT_GROUPS size"
    );
    ensure!(
        local_slice
            .iter()
            .all(|p| p.parent() == Some(Path::new("."))),
        "local candidates must reside in the current directory"
    );

    drop(home_guard);
    Ok(())
}
