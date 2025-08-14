//! Error types produced by the configuration loader.

use figment::error::Error as FigmentError;
use thiserror::Error;

/// Errors that can occur while loading configuration.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OrthoError {
    /// Error parsing command-line arguments.
    #[error("Failed to parse command-line arguments: {0}")]
    CliParsing(#[from] clap::Error),

    /// Error originating from a configuration file.
    #[error("Configuration file error in '{path}': {source}")]
    File {
        path: std::path::PathBuf,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Cycle detected while resolving `extends`.
    #[error("cyclic extends detected: {cycle}")]
    CyclicExtends { cycle: String },

    /// Error while gathering configuration from providers.
    #[error("Failed to gather configuration: {0}")]
    Gathering(#[from] figment::Error),

    /// Validation failures when building configuration.
    #[error("Validation failed for '{key}': {message}")]
    Validation { key: String, message: String },
}

impl From<OrthoError> for FigmentError {
    /// Allow using `?` in tests and examples that return `figment::Error`.
    fn from(e: OrthoError) -> Self {
        match e {
            // Preserve the original Figment error (keeps kind, metadata, and
            // sources).
            OrthoError::Gathering(fe) => fe,
            // Fall back to a message for other variants.
            other => FigmentError::from(other.to_string()),
        }
    }
}
