//! Core crate for the `OrthoConfig` configuration framework.
//!
//! This crate defines the [`OrthoConfig`] trait and supporting error types. The
//! actual implementation of the derive macro lives in the companion
//! `ortho_config_macros` crate.

pub use ortho_config_macros::OrthoConfig;

mod error;
mod file;
mod merge;
pub mod subcommand;
#[allow(deprecated)]
pub use merge::merge_cli_over_defaults;
#[allow(deprecated)]
pub use subcommand::{
    load_and_merge_subcommand, load_and_merge_subcommand_for, load_subcommand_config,
    load_subcommand_config_for,
};

/// Normalize a prefix by trimming trailing underscores and converting
/// to lowercase ASCII.
#[must_use]
pub fn normalize_prefix(prefix: &str) -> String {
    prefix.trim_end_matches('_').to_ascii_lowercase()
}

pub use error::OrthoError;
pub use file::load_config_file;

/// Trait implemented for structs that represent application configuration.
pub trait OrthoConfig: Sized + serde::de::DeserializeOwned {
    /// Loads, merges, and deserializes configuration from all available
    /// sources according to predefined precedence rules.
    #[allow(clippy::result_large_err)]
    ///
    /// # Errors
    ///
    /// Returns an [`OrthoError`] if gathering or deserialization fails.
    fn load() -> Result<Self, OrthoError>;

    /// Prefix used for environment variables and subcommand configuration.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    fn prefix() -> &'static str {
        ""
    }
}
