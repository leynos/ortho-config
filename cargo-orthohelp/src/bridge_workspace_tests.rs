//! Generated input ownership and stable compiler cache regressions.

use super::*;
use crate::cache::CacheKey;
use crate::metadata::{OrthoConfigDependency, PackageSelection};
use anyhow::{Context, Result, ensure};

fn selection(root: &camino::Utf8Path) -> PackageSelection {
    PackageSelection {
        package_name: "example".to_owned(),
        package_root: root.to_path_buf(),
        target_directory: root.join("configured-target"),
        package_version: "0.1.0".to_owned(),
        root_type: "example::Config".to_owned(),
        locales: None,
        windows: None,
        policy: None,
        ortho_config_dependency: OrthoConfigDependency {
            requirement: "0.9.0".to_owned(),
            path: None,
        },
    }
}

fn cache_key(fingerprint: &str) -> CacheKey {
    CacheKey {
        fingerprint: fingerprint.to_owned(),
        root_type: "example::Config".to_owned(),
        tool_version: "0.9.1".to_owned(),
        ir_version: "1".to_owned(),
        lockfile_hash: None,
    }
}

#[test]
fn different_cache_keys_reuse_the_cargo_selected_target() -> Result<()> {
    let state = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(state.path()).context("UTF-8 test directory")?;
    let package = selection(root);
    let first = super::super::prepare_cache(&package, &cache_key("first source body"));
    let second = super::super::prepare_cache(&package, &cache_key("changed source body"));
    ensure!(first.cache_dir != second.cache_dir);
    ensure!(first.target_dir == package.target_directory);
    ensure!(second.target_dir == package.target_directory);
    Ok(())
}

#[test]
fn concurrent_invocations_own_private_inputs_with_shared_build_state() -> Result<()> {
    let state = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(state.path()).context("UTF-8 test directory")?;
    let package = selection(root);
    let cache = super::super::prepare_cache(&package, &cache_key("same source body"));
    let first = BridgeWorkspace::new(&cache)?;
    let second = BridgeWorkspace::new(&cache)?;
    ensure!(first.paths.bridge_dir != second.paths.bridge_dir);
    ensure!(first.paths.target_dir == second.paths.target_dir);
    let first_dir = Dir::open_ambient_dir(&first.paths.bridge_dir, ambient_authority())?;
    let second_dir = Dir::open_ambient_dir(&second.paths.bridge_dir, ambient_authority())?;
    first_dir.write("Cargo.toml", "first caller")?;
    second_dir.write("Cargo.toml", "second caller")?;
    ensure!(first_dir.read_to_string("Cargo.toml")? == "first caller");
    ensure!(second_dir.read_to_string("Cargo.toml")? == "second caller");
    let cache_directory = Dir::open_ambient_dir(&cache.cache_dir, ambient_authority())?;
    let first_name = first
        .paths
        .bridge_dir
        .file_name()
        .context("private directory name")?
        .to_owned();
    drop(first_dir);
    drop(first);
    ensure!(!cache_directory.try_exists(&first_name)?);
    ensure!(second_dir.read_to_string("Cargo.toml")? == "second caller");
    Ok(())
}

#[rstest::rstest]
#[case::existing_lockfile(Some("version = 4\n"))]
#[case::no_lockfile(None)]
fn workspace_lockfile_is_seeded_privately(#[case] contents: Option<&str>) -> Result<()> {
    let state = tempfile::tempdir()?;
    let root = camino::Utf8Path::from_path(state.path()).context("UTF-8 test directory")?;
    let package = selection(root);
    let cache = super::super::prepare_cache(&package, &cache_key("source body"));
    let directory = Dir::open_ambient_dir(root, ambient_authority())?;
    if let Some(body) = contents {
        directory.write("Cargo.lock", body)?;
    }
    let config = BridgeConfig {
        workspace_root: root.to_path_buf(),
        package_root: package.package_root,
        package_name: package.package_name,
        root_type: package.root_type,
        ortho_config_dependency: package.ortho_config_dependency,
    };
    let workspace = BridgeWorkspace::new(&cache)?;
    workspace.seed_lockfile(&config)?;
    let private = Dir::open_ambient_dir(&workspace.paths.bridge_dir, ambient_authority())?;
    ensure!(private.try_exists("Cargo.lock")? == contents.is_some());
    if let Some(body) = contents {
        ensure!(private.read_to_string("Cargo.lock")? == body);
        private.write("Cargo.lock", "the generated root updates only its own copy")?;
        ensure!(directory.read_to_string("Cargo.lock")? == body);
    }
    Ok(())
}
