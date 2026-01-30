//! Cargo metadata discovery for `cargo-orthohelp`.

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, MetadataCommand, Package};
use serde::Deserialize;

use crate::cli::Args;
use crate::error::OrthohelpError;

/// Deserialised `package.metadata.ortho_config` defaults.
#[derive(Debug, Default, Deserialize)]
pub struct OrthoConfigMetadata {
    /// Default root type path for the configuration schema.
    pub root_type: Option<String>,
    /// Supported locales for documentation output.
    pub locales: Option<Vec<String>>,
}

/// Captures the `ortho_config` dependency requirements for the target crate.
#[derive(Debug, Clone)]
pub struct OrthoConfigDependency {
    /// Cargo version requirement string.
    pub requirement: String,
    /// Optional path override for workspace dependencies.
    pub path: Option<Utf8PathBuf>,
}

/// Summary of the selected package and doc generation inputs.
#[derive(Debug, Clone)]
pub struct PackageSelection {
    /// Selected Cargo package name.
    pub package_name: String,
    /// Root directory containing the package manifest.
    pub package_root: Utf8PathBuf,
    /// Cargo target directory for build artefacts.
    pub target_directory: Utf8PathBuf,
    /// Normalised root type path used by the bridge.
    pub root_type: String,
    /// Locales declared in package metadata, if any.
    pub locales: Option<Vec<String>>,
    /// Resolved `ortho_config` dependency metadata.
    pub ortho_config_dependency: OrthoConfigDependency,
}

/// Loads Cargo metadata for the current workspace.
pub fn load_metadata() -> Result<Metadata, OrthohelpError> {
    let mut command = MetadataCommand::new();
    command.no_deps();
    Ok(command.exec()?)
}

/// Selects the target package and resolves metadata defaults.
pub fn select_package(
    metadata: &Metadata,
    args: &Args,
) -> Result<PackageSelection, OrthohelpError> {
    if args.is_lib && args.bin.is_some() {
        return Err(OrthohelpError::Message(
            "cannot use --lib and --bin together".to_owned(),
        ));
    }

    let package = match args.package.as_ref() {
        Some(name) => find_package(metadata, name)?,
        None => metadata
            .root_package()
            .ok_or(OrthohelpError::WorkspaceRootMissing)?,
    };

    let package_name = package.name.clone();
    let package_root = package
        .manifest_path
        .parent()
        .map(Utf8Path::to_path_buf)
        .ok_or_else(|| OrthohelpError::Message("package manifest has no parent".to_owned()))?;
    let target_directory = metadata.target_directory.clone();
    let crate_ident = package_name.replace('-', "_");

    let metadata_defaults = parse_ortho_config_metadata(package)?;
    let raw_root_type = args
        .root_type
        .clone()
        .or_else(|| metadata_defaults.root_type.clone())
        .ok_or(OrthohelpError::MissingRootType)?;
    let root_type = normalize_root_type(&raw_root_type, &crate_ident);

    ensure_library_target(package)?;
    if let Some(bin) = args.bin.as_ref() {
        ensure_bin_target(package, bin)?;
    }

    let ortho_config_dependency = find_ortho_config_dependency(package)?;

    Ok(PackageSelection {
        package_name,
        package_root,
        target_directory,
        root_type,
        locales: metadata_defaults.locales,
        ortho_config_dependency,
    })
}

fn find_package<'a>(metadata: &'a Metadata, name: &str) -> Result<&'a Package, OrthohelpError> {
    metadata
        .packages
        .iter()
        .find(|package| package.name == name)
        .ok_or_else(|| OrthohelpError::PackageNotFound(name.to_owned()))
}

fn parse_ortho_config_metadata(package: &Package) -> Result<OrthoConfigMetadata, OrthohelpError> {
    let Some(value) = package.metadata.get("ortho_config") else {
        return Ok(OrthoConfigMetadata::default());
    };

    serde_json::from_value(value.clone()).map_err(OrthohelpError::MetadataJson)
}

fn ensure_library_target(package: &Package) -> Result<(), OrthohelpError> {
    let has_lib = package
        .targets
        .iter()
        .any(|target| target.kind.iter().any(|kind| kind == "lib"));
    if has_lib {
        Ok(())
    } else {
        Err(OrthohelpError::MissingLibraryTarget(package.name.clone()))
    }
}

fn ensure_bin_target(package: &Package, bin: &str) -> Result<(), OrthohelpError> {
    let has_bin = package
        .targets
        .iter()
        .any(|target| target.name == bin && target.kind.iter().any(|kind| kind == "bin"));
    if has_bin {
        return Ok(());
    }

    Err(OrthohelpError::MissingBinTarget {
        package: package.name.clone(),
        bin: bin.to_owned(),
    })
}

fn find_ortho_config_dependency(
    package: &Package,
) -> Result<OrthoConfigDependency, OrthohelpError> {
    let dependency = package
        .dependencies
        .iter()
        .find(|dep| dep.name == "ortho_config")
        .ok_or_else(|| OrthohelpError::MissingOrthoConfigDependency(package.name.clone()))?;

    Ok(OrthoConfigDependency {
        requirement: dependency.req.to_string(),
        path: dependency.path.clone(),
    })
}

fn normalize_root_type(raw: &str, crate_ident: &str) -> String {
    if let Some(stripped) = raw.strip_prefix("crate::") {
        return format!("{crate_ident}::{stripped}");
    }

    if raw.contains("::") {
        return raw.to_owned();
    }

    format!("{crate_ident}::{raw}")
}

#[cfg(test)]
mod tests {
    //! Unit tests for metadata helpers.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::crate_prefix("crate::Config", "demo", "demo::Config")]
    #[case::bare_type("Config", "demo", "demo::Config")]
    #[case::qualified("demo::Config", "ignored", "demo::Config")]
    fn normalizes_root_type(#[case] raw: &str, #[case] crate_ident: &str, #[case] expected: &str) {
        let normalized = normalize_root_type(raw, crate_ident);
        assert_eq!(normalized, expected);
    }
}
