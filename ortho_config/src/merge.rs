//! Utilities for merging command-line arguments with configuration defaults.

use figment::{Figment, providers::Serialized};
use serde::{Serialize, de::DeserializeOwned};

/// Merge CLI-provided values over application defaults using Figment.
///
/// Any field set to `None` in the `cli` argument will leave the corresponding
/// value from `defaults` intact. This function is intended for simple
/// "CLI over defaults" merging in example code and small projects.
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::merge_cli_over_defaults;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Default, Serialize, Deserialize)]
/// struct Config {
///     count: Option<u32>,
/// }
///
/// let defaults = Config { count: Some(1) };
/// let cli = Config { count: Some(2) };
/// let merged = merge_cli_over_defaults(&defaults, &cli).unwrap();
/// assert_eq!(merged.count, Some(2));
/// ```
///
/// # Errors
///
/// Returns any [`figment::Error`] produced while extracting the merged
/// configuration.
#[deprecated(note = "use `load_and_merge_subcommand` instead", since = "0.4.0")]
#[allow(clippy::result_large_err)]
pub fn merge_cli_over_defaults<T>(defaults: &T, cli: &T) -> Result<T, figment::Error>
where
    T: Serialize + DeserializeOwned + Default,
{
    Figment::from(Serialized::defaults(defaults))
        .merge(Serialized::defaults(cli))
        .extract()
}
