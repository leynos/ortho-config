//! Utilities for merging command-line arguments with configuration defaults.

use crate::OrthoError;
use figment::{Figment, providers::Serialized};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Recursively remove all [`Value::Null`] entries, pruning empty objects.
///
/// - Object fields equal to null are removed.
/// - Nested objects containing no non-null fields are also removed so empty
///   `#[clap(flatten)]` groups do not clobber defaults.
/// - Array elements equal to null are removed, dropping `None` entries in
///   `Vec<_>` but retaining empty arrays to allow deliberate clearing.
///
/// This is intended for CLI sanitisation so unset [`Option`] fields and
/// untouched flattened structs do not override defaults from files or
/// environment variables.
///
/// Returns `true` if `value` becomes empty after pruning (that is, it is
/// `Null` or an object with no remaining fields). Arrays never return `true`,
/// even when emptied, to preserve explicit clearing semantics.
fn strip_nulls(value: &mut Value) -> bool {
    match value {
        Value::Object(map) => {
            map.retain(|_, v| !strip_nulls(v));
            map.is_empty()
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                if strip_nulls(v) {
                    *v = Value::Null;
                }
            }
            arr.retain(|v| !v.is_null());
            false
        }
        Value::Null => true,
        _ => false,
    }
}

/// Serialize a CLI struct to JSON, removing fields set to `None`.
///
/// # Examples
///
/// ```rust
/// use ortho_config::value_without_nones;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Args { count: Option<u32> }
///
/// let v = value_without_nones(&Args { count: None }).unwrap();
/// assert_eq!(v, serde_json::json!({}));
/// ```
///
/// # Errors
///
/// Returns any [`serde_json::Error`] encountered during serialization.
pub fn value_without_nones<T: Serialize>(cli: &T) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(cli)?;
    let _ = strip_nulls(&mut value);
    Ok(value)
}

/// Convert a [`serde_json::Error`] into [`OrthoError::Gathering`].
///
/// This helper is used by [`sanitize_value`] to map JSON serialisation
/// failures into the library's error type.
///
/// # Examples
///
/// ```rust
/// use ortho_config::sanitize_value;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Args { count: Option<u32> }
///
/// // Sanitise `Args` and map serialisation errors to `OrthoError::Gathering`.
/// sanitize_value(&Args { count: None }).unwrap();
/// ```
fn convert_gathering_error(e: &serde_json::Error) -> OrthoError {
    OrthoError::Gathering(figment::Error::from(e.to_string()))
}

/// Serialize `value` to JSON, pruning `None` fields and mapping errors to
/// [`OrthoError`].
///
/// # Examples
///
/// ```rust
/// use ortho_config::sanitize_value;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Args { count: Option<u32> }
///
/// let v = sanitize_value(&Args { count: None }).unwrap();
/// assert_eq!(v, serde_json::json!({}));
/// ```
///
/// # Errors
///
/// Returns an [`OrthoError`] if JSON serialization fails.
#[expect(
    clippy::result_large_err,
    reason = "Return OrthoError to keep a single error type across the public API"
)]
pub fn sanitize_value<T: Serialize>(value: &T) -> Result<Value, OrthoError> {
    value_without_nones(value).map_err(|e| convert_gathering_error(&e))
}

/// Produce a Figment provider from `value` with `None` fields removed.
///
/// This helper wraps [`sanitize_value`] and avoids repeating the
/// `Serialized::defaults` pattern when layering providers.
///
/// # Examples
///
/// ```rust
/// use figment::Figment;
/// use ortho_config::sanitized_provider;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Args { count: Option<u32> }
///
/// let provider = sanitized_provider(&Args { count: None }).unwrap();
/// let value: serde_json::Value = Figment::from(provider).extract().unwrap();
/// assert_eq!(value, serde_json::json!({}));
/// ```
///
/// # Errors
///
/// Returns an [`OrthoError`] if JSON serialization fails.
#[expect(
    clippy::result_large_err,
    reason = "Return OrthoError to keep a single error type across the public API"
)]
pub fn sanitized_provider<T: Serialize>(
    value: &T,
) -> Result<Serialized<serde_json::Value>, OrthoError> {
    sanitize_value(value).map(Serialized::defaults)
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
#[expect(
    clippy::result_large_err,
    reason = "Return figment::Error for backward compatibility"
)]
pub fn merge_cli_over_defaults<T>(defaults: &T, cli: &T) -> Result<T, figment::Error>
where
    T: Serialize + DeserializeOwned + Default,
{
    let cli_value = value_without_nones(cli).map_err(|e| figment::Error::from(e.to_string()))?;
    Figment::from(Serialized::defaults(defaults))
        .merge(Serialized::defaults(&cli_value))
        .extract()
}
