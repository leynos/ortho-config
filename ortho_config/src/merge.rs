//! Utilities for merging command-line arguments with configuration defaults.

use figment::{Figment, providers::Serialized};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

fn strip_nulls(value: &mut Value) {
    match value {
        Value::Object(map) => {
            let keys: Vec<_> = map
                .iter()
                .filter(|(_, v)| v.is_null())
                .map(|(k, _)| k.clone())
                .collect();
            for key in keys {
                map.remove(&key);
            }
            for v in map.values_mut() {
                strip_nulls(v);
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(strip_nulls),
        _ => {}
    }
}

/// Serialise a CLI struct to JSON, removing fields set to `None`.
///
/// # Errors
///
/// Returns any [`serde_json::Error`] encountered during serialisation.
pub fn value_without_nones<T: Serialize>(cli: &T) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(cli)?;
    strip_nulls(&mut value);
    Ok(value)
}

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
/// let merged = merge_cli_over_defaults(&defaults, &cli)
///     .expect("failed to merge configuration");
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
    let cli_value = value_without_nones(cli).map_err(|e| figment::Error::from(e.to_string()))?;
    Figment::from(Serialized::defaults(defaults))
        .merge(Serialized::defaults(&cli_value))
        .extract()
}
