#![cfg_attr(docsrs, feature(doc_cfg))]

//! Core crate for the `OrthoConfig` configuration framework.
//!
//! Defines the [`OrthoConfig`] trait, error types and sanitization helpers used
//! to layer configuration from the CLI, files and environment. The derive macro
//! lives in the companion `ortho_config_macros` crate.

pub use ortho_config_macros::OrthoConfig;

pub use figment;
#[cfg(feature = "json5")]
#[cfg_attr(docsrs, doc(cfg(feature = "json5")))]
pub use figment_json5;
#[cfg(feature = "json5")]
#[cfg_attr(docsrs, doc(cfg(feature = "json5")))]
pub use json5;
pub use serde_json;
#[cfg(feature = "yaml")]
#[cfg_attr(docsrs, doc(cfg(feature = "yaml")))]
pub use serde_yaml;
#[cfg(feature = "toml")]
#[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
pub use toml;
pub use uncased;
#[cfg(any(unix, target_os = "redox"))]
#[cfg_attr(docsrs, doc(cfg(any(unix, target_os = "redox"))))]
pub use xdg;

mod csv_env;
pub mod declarative;
pub mod discovery;
mod error;
pub mod file;
mod merge;
mod result_ext;
pub mod subcommand;
pub use crate::subcommand::SubcmdConfigMerge;
pub use result_ext::{IntoFigmentError, OrthoMergeExt, OrthoResultExt, ResultIntoFigment};
pub use subcommand::{load_and_merge_subcommand, load_and_merge_subcommand_for};

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
pub use declarative::{DeclarativeMerge, MergeComposer, MergeLayer, MergeProvenance};
pub use discovery::{ConfigDiscovery, ConfigDiscoveryBuilder, DiscoveryLoadOutcome};
pub use error::OrthoError;
pub use file::load_config_file;
/// Re-export sanitization helpers used to strip `None` fields and produce a
/// Figment provider.
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::{sanitize_value, sanitized_provider, OrthoResult};
/// #[derive(serde::Serialize)]
/// struct CLI { flag: Option<()> }
///
/// # fn main() -> OrthoResult<()> {
/// let cli = CLI { flag: None };
/// let provider = sanitized_provider(&cli)?; // ready to merge over defaults
/// let _json = sanitize_value(&cli)?;        // raw serialized value with `None`s removed
/// # let _ = provider;
/// # Ok(())
/// # }
/// ```
pub use merge::{sanitize_value, sanitized_provider, value_without_nones};
use std::sync::Arc;

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
    /// use ortho_config::{OrthoConfig, OrthoResult};
    /// use serde::Deserialize;
    ///
    /// #[derive(Deserialize, OrthoConfig)]
    /// struct AppConfig {
    ///     port: u16,
    /// }
    ///
    /// # fn main() -> OrthoResult<()> {
    /// let _cfg = AppConfig::load()?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`crate::OrthoError`] if parsing command-line arguments, reading
    /// files or deserializing configuration fails.
    fn load() -> OrthoResult<Self> {
        Self::load_from_iter(std::env::args_os())
    }

    /// Loads configuration from the provided iterator of command-line
    /// arguments.
    ///
    /// # Errors
    ///
    /// Returns an [`crate::OrthoError`] if parsing command-line arguments, reading
    /// files or deserializing configuration fails.
    fn load_from_iter<I, T>(iter: I) -> OrthoResult<Self>
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

/// Canonical result type for public APIs in this crate.
///
/// Errors are wrapped in an `Arc` to reduce the size of `Result` and avoid
/// `clippy::result_large_err` on public signatures while keeping rich error
/// variants internally. This keeps call-sites lightweight and encourages cheap
/// cloning while propagating errors.
pub type OrthoResult<T> = std::result::Result<T, Arc<OrthoError>>;
