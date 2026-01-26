//! Error types for `cargo-orthohelp`.

use camino::Utf8PathBuf;
use thiserror::Error;

/// Errors surfaced by the `cargo-orthohelp` pipeline.
#[derive(Debug, Error)]
pub enum OrthohelpError {
    #[error("cargo metadata failed: {0}")]
    Metadata(#[from] cargo_metadata::Error),

    #[error("failed to parse IR JSON: {0}")]
    IrJson(#[from] serde_json::Error),

    #[error("failed to parse locale '{value}': {message}")]
    InvalidLocale { value: String, message: String },

    #[error("package '{0}' not found in workspace")]
    PackageNotFound(String),

    #[error("workspace root package was not available; pass --package")]
    WorkspaceRootMissing,

    #[error("root type missing; pass --root-type or set package.metadata.ortho_config.root_type")]
    MissingRootType,

    #[error(
        "package '{0}' does not define a library target; move the config type into a library crate"
    )]
    MissingLibraryTarget(String),

    #[error("binary target '{bin}' not found in package '{package}'")]
    MissingBinTarget { package: String, bin: String },

    #[error("dependency 'ortho_config' not found in package '{0}'")]
    MissingOrthoConfigDependency(String),

    #[error("unsupported format '{0}'; only 'ir' output is available in this release")]
    UnsupportedFormat(String),

    #[error("cached IR missing at {0}")]
    MissingCache(Utf8PathBuf),

    #[error("bridge build failed (status {status}): {message}")]
    BridgeBuildFailure { status: i32, message: String },

    #[error("bridge execution failed (status {status}): {message}")]
    BridgeExecutionFailure { status: i32, message: String },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: Utf8PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Message(String),
}
