//! Primary error enum for configuration loading flows.

use figment::Error as FigmentError;
use thiserror::Error;

use super::aggregate::AggregatedErrors;

/// Errors that can occur while loading configuration.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OrthoError {
    /// Error parsing command-line arguments.
    #[error("Failed to parse command-line arguments: {0}")]
    CliParsing(#[from] Box<clap::Error>),

    /// Error originating from a configuration file.
    #[error("Configuration file error in '{path}': {source}")]
    File {
        /// Path that triggered the configuration failure.
        path: std::path::PathBuf,
        /// Underlying error reported by the file loader.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Cycle detected while resolving `extends`.
    #[error("cyclic extends detected: {cycle}")]
    CyclicExtends {
        /// Chain of configuration files participating in the cycle.
        cycle: String,
    },

    /// Error while gathering configuration from providers.
    #[error("Failed to gather configuration: {0}")]
    Gathering(#[from] Box<FigmentError>),

    /// Failure merging CLI values over configuration sources.
    #[error("Failed to merge CLI with configuration: {source}")]
    Merge {
        /// Underlying error describing the merge failure.
        #[source]
        source: Box<FigmentError>,
    },

    /// Validation failures when building configuration.
    #[error("Validation failed for '{key}': {message}")]
    Validation {
        /// Configuration key that failed validation.
        key: String,
        /// Human-readable explanation of the validation failure.
        message: String,
    },

    /// Multiple errors occurred while loading configuration.
    #[error("multiple configuration errors:\n{0}")]
    Aggregate(Box<AggregatedErrors>),
}
