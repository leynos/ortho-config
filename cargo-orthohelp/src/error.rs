//! Error types for `cargo-orthohelp`.

use camino::Utf8PathBuf;
use thiserror::Error;

use crate::roff::InvalidManSection;

/// Errors surfaced by the `cargo-orthohelp` pipeline.
#[derive(Debug, Error)]
pub enum OrthohelpError {
    /// Failed to invoke or parse `cargo metadata`.
    #[error("cargo metadata failed: {0}")]
    Metadata(#[from] cargo_metadata::Error),

    /// Failed to parse intermediate representation JSON.
    #[error("failed to parse IR JSON: {0}")]
    IrJson(#[from] serde_json::Error),

    /// Failed to parse package metadata JSON from Cargo.toml.
    #[error("failed to parse package metadata JSON: {0}")]
    MetadataJson(serde_json::Error),

    /// The provided locale identifier could not be parsed.
    #[error("failed to parse locale '{value}': {message}")]
    InvalidLocale {
        /// The invalid locale value.
        value: String,
        /// A description of why the locale is invalid.
        message: String,
    },

    /// The man page section number is outside the valid range (1-8).
    #[error("{0}")]
    InvalidManSection(#[from] InvalidManSection),

    /// The specified package was not found in the workspace.
    #[error("package '{0}' not found in workspace")]
    PackageNotFound(String),

    /// No root package is available; `--package` must be specified.
    #[error("workspace root package was not available; pass --package")]
    WorkspaceRootMissing,

    /// The config root type is not specified anywhere.
    #[error("root type missing; pass --root-type or set package.metadata.ortho_config.root_type")]
    MissingRootType,

    /// The target package does not have a library target.
    #[error(
        "package '{0}' does not define a library target; move the config type into a library crate"
    )]
    MissingLibraryTarget(String),

    /// The specified binary target was not found in the package.
    #[error("binary target '{bin}' not found in package '{package}'")]
    MissingBinTarget {
        /// The package name.
        package: String,
        /// The missing binary name.
        bin: String,
    },

    /// The package does not depend on `ortho_config`.
    #[error("dependency 'ortho_config' not found in package '{0}'")]
    MissingOrthoConfigDependency(String),

    /// The requested output format is not supported.
    #[error("unsupported format '{0}'; supported formats are: ir, man, ps, all")]
    UnsupportedFormat(String),

    /// Cached IR was expected but not found.
    #[error("cached IR missing at {0}")]
    MissingCache(Utf8PathBuf),

    /// The bridge crate failed to build.
    #[error("bridge build failed (status {status}): {message}")]
    BridgeBuildFailure {
        /// Exit status code.
        status: i32,
        /// Error message from the build.
        message: String,
    },

    /// The bridge crate failed during execution.
    #[error("bridge execution failed (status {status}): {message}")]
    BridgeExecutionFailure {
        /// Exit status code.
        status: i32,
        /// Error message from execution.
        message: String,
    },

    /// A filesystem I/O error occurred.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// The path where the error occurred.
        path: Utf8PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// A generic error message.
    #[error("{0}")]
    Message(String),
}
