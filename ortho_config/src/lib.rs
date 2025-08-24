//! Core crate for the `OrthoConfig` configuration framework.
//!
//! This crate defines the [`OrthoConfig`] trait and supporting error types. The
//! actual implementation of the derive macro lives in the companion
//! `ortho_config_macros` crate.

pub use ortho_config_macros::OrthoConfig;

mod csv_env;
mod error;
mod file;
mod merge;
pub mod subcommand;
#[expect(deprecated, reason = "Retain helper for backwards compatibility")]
pub use merge::merge_cli_over_defaults;
pub use merge::{sanitize_value, sanitized_provider, value_without_nones};
pub use subcommand::{SubcmdConfigMerge, load_and_merge_subcommand};

#[expect(
    deprecated,
    reason = "Re-export deprecated subcommand helpers for back-compat. FIXME: remove in the next minor release"
)]
pub use subcommand::{load_subcommand_config, load_subcommand_config_for};

/// Normalize a prefix by trimming trailing underscores and converting
/// to lowercase ASCII.
///
/// # Examples
///
/// ```rust
/// use ortho_config::normalize_prefix;
///
/// assert_eq!(normalize_prefix("FOO__"), "foo");
/// assert_eq!(normalize_prefix("foo"), "foo");
/// assert_eq!(normalize_prefix("Another_App_"), "another_app");
/// assert_eq!(normalize_prefix("___"), "");
/// assert_eq!(normalize_prefix("FÖÖ_"), "fÖÖ"); // ASCII-only lowercasing; non-ASCII remains unchanged
/// ```
#[must_use]
pub fn normalize_prefix(prefix: &str) -> String {
    prefix.trim_end_matches('_').to_ascii_lowercase()
}

pub use csv_env::CsvEnv;
pub use error::OrthoError;
pub use file::load_config_file;

/// Trait implemented for structs that represent application configuration.
pub trait OrthoConfig: Sized + serde::de::DeserializeOwned {
    /// Loads configuration from command-line arguments, environment variables
    /// and configuration files using the standard precedence rules.
    ///
    /// Command-line arguments have the highest precedence, followed by
    /// environment variables and finally configuration files. Default values
    /// specified via `#[ortho_config(default = ...)]` sit at the lowest
    /// precedence level.
    ///
    /// ```rust,no_run
    /// use ortho_config::{OrthoConfig, OrthoError};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize, OrthoConfig)]
    /// struct AppConfig {
    ///     port: u16,
    /// }
    ///
    /// # fn main() -> Result<(), OrthoError> {
    /// let _cfg = AppConfig::load()?;
    /// # Ok(())
    /// # }
    /// ```
    #[expect(
        clippy::result_large_err,
        reason = "Return OrthoError to keep a single error type across the public API"
    )]
    ///
    /// # Errors
    ///
    /// Returns an [`OrthoError`] if parsing command-line arguments, reading
    /// files or deserializing configuration fails.
    fn load() -> Result<Self, OrthoError> {
        Self::load_from_iter(std::env::args_os())
    }

    /// Loads configuration from the provided iterator of command-line
    /// arguments.
    #[expect(
        clippy::result_large_err,
        reason = "Return OrthoError to keep a single error type across the public API"
    )]
    ///
    /// # Errors
    ///
    /// Returns an [`OrthoError`] if parsing command-line arguments, reading
    /// files or deserializing configuration fails.
    fn load_from_iter<I, T>(iter: I) -> Result<Self, OrthoError>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone;

    /// Prefix used for environment variables and subcommand configuration.
    #[must_use]
    // Intentionally non-const so implementations can read runtime information.
    fn prefix() -> &'static str {
        ""
    }
}
