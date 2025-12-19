#![cfg_attr(docsrs, feature(doc_cfg))]

//! Core crate for the `OrthoConfig` configuration framework.
//!
//! Defines the [`OrthoConfig`] trait, error types and sanitization helpers used
//! to layer configuration from the CLI, files and environment. The derive macro
//! lives in the companion `ortho_config_macros` crate.

#[cfg(all(feature = "yaml", not(feature = "serde_json")))]
compile_error!("The `serde_json` feature must be enabled when `yaml` support is active.");

pub use ortho_config_macros::OrthoConfig;
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub use ortho_config_macros::SelectedSubcommandMerge;

pub use figment;
#[cfg(feature = "json5")]
#[cfg_attr(docsrs, doc(cfg(feature = "json5")))]
pub use figment_json5;
#[cfg(feature = "json5")]
#[cfg_attr(docsrs, doc(cfg(feature = "json5")))]
pub use json5;
#[cfg(feature = "serde_json")]
pub use serde_json;
#[cfg(feature = "yaml")]
#[cfg_attr(docsrs, doc(cfg(feature = "yaml")))]
pub use serde_saphyr;
#[cfg(feature = "toml")]
#[cfg_attr(docsrs, doc(cfg(feature = "toml")))]
pub use toml;
pub use uncased;
#[cfg(any(unix, target_os = "redox"))]
#[cfg_attr(docsrs, doc(cfg(any(unix, target_os = "redox"))))]
pub use xdg;

mod csv_env;
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub mod declarative;
pub mod discovery;
mod error;
pub mod file;
mod localizer;
mod merge;
mod post_merge;
mod result_ext;
pub mod subcommand;
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub use crate::subcommand::SubcmdConfigMerge;
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub use result_ext::OrthoJsonMergeExt;
pub use result_ext::{IntoFigmentError, OrthoMergeExt, OrthoResultExt, ResultIntoFigment};
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub use subcommand::{
    LoadGlobalsAndSelectedSubcommandError, SelectedSubcommandMerge, SelectedSubcommandMergeError,
    load_globals_and_merge_selected_subcommand,
};
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub use subcommand::{
    load_and_merge_subcommand, load_and_merge_subcommand_for,
    load_and_merge_subcommand_for_with_matches, load_and_merge_subcommand_with_matches,
};

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
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub use declarative::{DeclarativeMerge, MergeComposer, MergeLayer, MergeProvenance};
pub use discovery::{ConfigDiscovery, ConfigDiscoveryBuilder, DiscoveryLoadOutcome};
pub use error::{OrthoError, is_display_request};
pub use file::load_config_file;
pub use localizer::{
    FluentBundleSource, FluentLocalizer, FluentLocalizerBuilder, FluentLocalizerError,
    FormattingIssue, LocalizationArgs, Localizer, NoOpLocalizer, clap_error_formatter,
    localize_clap_error, localize_clap_error_with_command,
};
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
#[cfg(feature = "serde_json")]
pub use merge::{CliValueExtractor, sanitize_value, sanitized_provider, value_without_nones};
pub use post_merge::{PostMergeContext, PostMergeHook};
use std::sync::Arc;
pub use unic_langid::{LanguageIdentifier, langid};

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
