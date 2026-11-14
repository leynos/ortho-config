//! Manifest rendering and application command policy for owned doc workspaces.

use super::{DependencyAlias, DocumentedExample};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::toml;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

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

/// Render the generated example workspace's manifest, naming `ortho_config`
/// under `dependency_name` so aliased examples compile against the real crate.
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
            "tracing = \"0.1\"\n",
            "tracing-subscriber = {{ version = \"0.3\", features = [\"env-filter\"] }}\n",
        ),
        dependency_name, paths.ortho_config, paths.ortho_l10n, paths.cargo_orthohelp,
    )
}

/// Create a temporary package directory holding `manifest`.
///
/// The package lives under the repository's documentation-workspace root so
/// Cargo's resolved target and the shared build cache apply to it.
pub(super) fn create_package(manifest: String) -> Result<(TempDir, Dir)> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("find documentation repository")?;
    let workspaces = repository.join("target/documentation-workspaces");
    Dir::create_ambient_dir_all(&workspaces, ambient_authority())?;
    let root = tempfile::Builder::new()
        .prefix("example-")
        .tempdir_in(workspaces)
        .context("create documentation example workspace")?;
    let directory = Dir::open_ambient_dir(root.path(), ambient_authority())
        .context("open documentation example workspace")?;
    directory
        .create_dir_all("src/bin")
        .context("create bin directory")?;
    directory
        .write("Cargo.toml", manifest)
        .context("write example manifest")?;
    seed_lockfile(repository, &directory)?;
    Ok((root, directory))
}

/// Render a published dependency recipe as a buildable package manifest.
///
/// Only the `ortho_config` requirement changes: it keeps its published version
/// requirement and gains a path to this checkout, so the recipe's own metadata
/// and dependency settings are what the build exercises. An empty `[workspace]`
/// table keeps the package out of the repository workspace.
pub(super) fn recipe_manifest(recipe: &DocumentedExample) -> Result<String> {
    ensure!(recipe.language == "toml", "{} is not TOML", recipe.id);
    let mut manifest: toml::Value =
        toml::from_str(&recipe.body).with_context(|| format!("parse {}", recipe.id))?;
    let crate_path = toml::Value::String(env!("CARGO_MANIFEST_DIR").to_owned());
    let dependencies = manifest
        .get_mut("dependencies")
        .and_then(toml::Value::as_table_mut)
        .with_context(|| format!("{} should declare dependencies", recipe.id))?;
    let requirement = dependencies
        .get("ortho_config")
        .and_then(toml::Value::as_str)
        .with_context(|| format!("{} should require ortho_config", recipe.id))?
        .to_owned();
    let mut local = toml::map::Map::new();
    local.insert("version".to_owned(), toml::Value::String(requirement));
    local.insert("path".to_owned(), crate_path);
    dependencies.insert("ortho_config".to_owned(), toml::Value::Table(local));
    manifest
        .as_table_mut()
        .context("a recipe should be a TOML table")?
        .insert(
            "workspace".to_owned(),
            toml::Value::Table(toml::map::Map::new()),
        );
    toml::to_string(&manifest).with_context(|| format!("render {}", recipe.id))
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
