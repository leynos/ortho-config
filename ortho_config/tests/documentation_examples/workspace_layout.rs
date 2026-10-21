//! Manifest rendering and application command policy for owned doc workspaces.

use super::DependencyAlias;
use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs::Dir};
use std::path::Path;
use std::process::Command;

const CHILD_ENV_ALLOWLIST: &[&str] = &[
    "SYSTEMROOT",
    "WINDIR",
    "LLVM_PROFILE_FILE",
    "LLVM_PROFILE_MERGE_POOL_SIZE",
];

pub(super) fn sanitize_application_environment(command: &mut Command) {
    command.env_clear();
    for name in CHILD_ENV_ALLOWLIST {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
}

pub(super) fn manifest(DependencyAlias(dependency_name): DependencyAlias<'_>) -> String {
    let crate_path = toml::Value::String(env!("CARGO_MANIFEST_DIR").to_owned()).to_string();
    let sibling = |name: &str| {
        toml::Value::String(format!("{}/../{name}", env!("CARGO_MANIFEST_DIR"))).to_string()
    };
    render_manifest(
        dependency_name,
        &ManifestPaths {
            ortho_config: &crate_path,
            ortho_l10n: &sibling("ortho_l10n"),
            cargo_orthohelp: &sibling("cargo-orthohelp"),
        },
    )
}

/// TOML-serialized paths of the workspace packages a documented example uses.
pub(super) struct ManifestPaths<'a> {
    pub(super) ortho_config: &'a str,
    pub(super) ortho_l10n: &'a str,
    pub(super) cargo_orthohelp: &'a str,
}

/// Render the generated manifest from serialized Cargo dependency paths.
///
/// This stays private to the documentation workspace and its path regression
/// test; callers must serialize the path as a TOML value before using it.
pub(super) fn render_manifest(dependency_name: &str, paths: &ManifestPaths<'_>) -> String {
    format!(
        concat!(
            "[package]\n",
            "name = \"documented-examples\"\n",
            "version = \"0.0.0\"\n",
            "edition = \"2024\"\n\n",
            "[workspace]\n\n",
            "[dependencies]\n",
            "{} = {{ package = \"ortho_config\", path = {} }}\n",
            "ortho_l10n = {{ path = {} }}\n",
            "cargo_orthohelp = {{ package = \"cargo-orthohelp\", path = {} }}\n",
            "clap = {{ version = \"4.5\", features = [\"derive\"] }}\n",
            "serde = {{ version = \"1.0\", features = [\"derive\"] }}\n",
            "serde_json = \"1\"\n",
            "tracing-subscriber = {{ version = \"0.3\", features = [\"env-filter\"] }}\n",
        ),
        dependency_name, paths.ortho_config, paths.ortho_l10n, paths.cargo_orthohelp,
    )
}

/// Seed a private example lockfile without changing the repository's version pins.
///
/// This belongs only to documentation workspace layout. Cargo may update the
/// copy for the generated root while retaining compatible dependency versions.
pub(super) fn seed_lockfile(repository: &Path, workspace: &Dir) -> Result<()> {
    let source = Dir::open_ambient_dir(repository, ambient_authority())
        .context("open repository for documentation lockfile")?;
    if source.try_exists("Cargo.lock")? {
        source
            .copy("Cargo.lock", workspace, "Cargo.lock")
            .context("seed private documentation lockfile")?;
    }
    Ok(())
}
