//! Primary error enum for configuration loading flows.

use figment::Error as FigmentError;
use thiserror::Error;

use crate::profile::{AvailableProfileNames, ProfileSource};

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

    /// A selected profile does not exist in the file chain.
    #[error("unknown profile '{selected}' selected via {selection_source}: {available}")]
    UnknownProfile {
        /// The selected profile name that could not be found.
        selected: String,
        /// How the selection was supplied.
        ///
        /// Named `selection_source` because thiserror reserves the field name
        /// `source` for the error source chain, and this field holds
        /// selection metadata rather than an error.
        selection_source: ProfileSource,
        /// Sorted profile names the file chain defines, rendered capped.
        available: AvailableProfileNames,
    },

    /// A profile name violates the `[A-Za-z0-9_-]+` grammar.
    #[error("invalid profile name '{name}': names must match [A-Za-z0-9_-]+")]
    InvalidProfileName {
        /// The offending name.
        name: String,
    },

    /// A profile name is reserved.
    #[error("profile name '{name}' is reserved")]
    ReservedProfileName {
        /// The reserved name.
        name: String,
    },

    /// A profile table defines a key `OrthoConfig` reserves.
    #[error("profile '{profile}' must not define the forbidden key '{key}'")]
    ProfileForbiddenKey {
        /// The profile name.
        profile: String,
        /// The forbidden key.
        key: String,
    },

    /// Multiple errors occurred while loading configuration.
    #[error("multiple configuration errors:\n{0}")]
    Aggregate(Box<AggregatedErrors>),
}
