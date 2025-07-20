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
pub trait OrthoConfig: Sized + serde::de::DeserializeOwned + clap::Parser {
    /// Loads configuration from files and environment variables, then merges the
    /// command-line arguments from `self` over the top.
    ///
    /// This is the recommended way to load configuration. It requires the
    /// struct to also derive `clap::Parser`.
    ///
    /// ```rust,no_run
    /// # use ortho_config::{OrthoConfig, OrthoError};
    /// # use clap::Parser;
    /// # #[derive(Parser, OrthoConfig, serde::Deserialize, serde::Serialize)]
    /// # struct AppConfig {};
    /// # fn main() -> Result<(), OrthoError> {
    /// let cli_args = AppConfig::parse();
    /// let config = cli_args.load_and_merge()?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::result_large_err)]
    ///
    /// # Errors
    ///
    /// Returns an [`OrthoError`] if gathering or deserialization fails.
    fn load_and_merge(&self) -> Result<Self, OrthoError>
    where
        Self: serde::Serialize;

    /// DEPRECATED: Loads configuration by re-parsing command-line arguments.
    ///
    /// This method is inefficient and can cause issues with `clap` subcommands.
    /// It is recommended to use `load_and_merge` instead.
    #[deprecated(
        since = "0.4.0",
        note = "Use `YourConfig::parse().load_and_merge()` instead"
    )]
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
