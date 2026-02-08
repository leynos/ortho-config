//! Cargo metadata discovery for `cargo-orthohelp`.

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, MetadataCommand, Package};
use serde::Deserialize;

use crate::cli::Args;
use crate::error::OrthohelpError;
use crate::schema::WindowsMetadata;

/// Deserialised `package.metadata.ortho_config` defaults.
#[derive(Debug, Default, Deserialize)]
pub struct OrthoConfigMetadata {
    /// Default root type path for the configuration schema.
    pub root_type: Option<String>,
    /// Supported locales for documentation output.
    pub locales: Option<Vec<String>>,
    /// Optional Windows settings for `PowerShell` output.
    #[serde(default)]
    pub windows: Option<WindowsMetadataOverrides>,
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
    /// Package version string.
    pub package_version: String,
    /// Normalised root type path used by the bridge.
    pub root_type: String,
    /// Locales declared in package metadata, if any.
    pub locales: Option<Vec<String>>,
    /// Windows metadata overrides from Cargo.toml, if any.
    pub windows: Option<WindowsMetadataOverrides>,
    /// Resolved `ortho_config` dependency metadata.
    pub ortho_config_dependency: OrthoConfigDependency,
}

/// Optional Windows metadata overrides from Cargo.toml.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct WindowsMetadataOverrides {
    /// Module name used for `PowerShell` output.
    pub module_name: Option<String>,
    /// Aliases exported by the wrapper module.
    pub export_aliases: Option<Vec<String>>,
    /// Whether `CommonParameters` are included in help output.
    pub include_common_parameters: Option<bool>,
    /// Whether subcommands are split into wrapper functions.
    pub split_subcommands_into_functions: Option<bool>,
    /// Optional `HelpInfoUri` for Update-Help.
    pub help_info_uri: Option<String>,
}

impl WindowsMetadataOverrides {
    /// Resolves overrides against IR-provided Windows metadata.
    #[must_use]
    pub fn resolve(&self, base: Option<&WindowsMetadata>) -> ResolvedWindowsMetadata {
        let mut resolved = base
            .cloned()
            .map(ResolvedWindowsMetadata::from)
            .unwrap_or_default();
        resolved.module_name = self.module_name.clone().or(resolved.module_name);
        resolved.export_aliases = self
            .export_aliases
            .clone()
            .unwrap_or(resolved.export_aliases);
        resolved.include_common_parameters = self
            .include_common_parameters
            .unwrap_or(resolved.include_common_parameters);
        resolved.split_subcommands_into_functions = self
            .split_subcommands_into_functions
            .unwrap_or(resolved.split_subcommands_into_functions);
        resolved.help_info_uri = self.help_info_uri.clone().or(resolved.help_info_uri);

        resolved
    }
}

/// Fully resolved Windows metadata used for `PowerShell` output.
#[derive(Debug, Clone)]
pub struct ResolvedWindowsMetadata {
    /// Module name used for `PowerShell` output.
    pub module_name: Option<String>,
    /// Aliases exported by the wrapper module.
    pub export_aliases: Vec<String>,
    /// Whether `CommonParameters` are included in help output.
    pub include_common_parameters: bool,
    /// Whether subcommands are split into wrapper functions.
    pub split_subcommands_into_functions: bool,
    /// Optional `HelpInfoUri` for Update-Help.
    pub help_info_uri: Option<String>,
}

impl Default for ResolvedWindowsMetadata {
    fn default() -> Self {
        Self {
            module_name: None,
            export_aliases: Vec::new(),
            include_common_parameters: true,
            split_subcommands_into_functions: false,
            help_info_uri: None,
        }
    }
}

impl From<WindowsMetadata> for ResolvedWindowsMetadata {
    fn from(metadata: WindowsMetadata) -> Self {
        Self {
            module_name: metadata.module_name,
            export_aliases: metadata.export_aliases,
            include_common_parameters: metadata.include_common_parameters,
            split_subcommands_into_functions: metadata.split_subcommands_into_functions,
            help_info_uri: metadata.help_info_uri,
        }
    }
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
    let package_version = package.version.to_string();
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
        package_version,
        root_type,
        locales: metadata_defaults.locales,
        windows: metadata_defaults.windows,
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
