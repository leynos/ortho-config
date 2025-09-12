//! Helpers for sanitizing and merging command-line arguments with
//! configuration defaults.

use crate::{OrthoResult, OrthoResultExt};
use figment::providers::Serialized;
use serde::Serialize;
use serde_json::Value;

/// Recursively remove all [`Value::Null`] entries, pruning empty objects.
///
/// - Object fields equal to null are removed.
/// - Nested objects containing no non-null fields are also removed so empty
///   `#[clap(flatten)]` groups do not clobber defaults.
/// - Array elements equal to null are removed, dropping `None` entries in
///   `Vec<_>` but retaining empty arrays to allow deliberate clearing.
///
/// Intended for CLI sanitization so unset [`Option`] fields and untouched
/// flattened structs do not override defaults from files or environment
/// variables.
/// Arrays are never removed, even when emptied; this function only removes
/// [`Option::None`] fields.
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
/// let v = value_without_nones(&Args { count: None })
///     .expect("expected serialization to succeed");
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
/// let v = sanitize_value(&Args { count: None })
///     .expect("expected sanitization to succeed");
/// assert_eq!(v, serde_json::json!({}));
/// ```
///
/// # Errors
///
/// Returns an [`OrthoError`] if JSON serialization fails.
pub fn sanitize_value<T: Serialize>(value: &T) -> OrthoResult<Value> {
    value_without_nones(value).into_ortho()
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
/// let provider = sanitized_provider(&Args { count: None })
///     .expect("expected provider creation to succeed");
/// let value: serde_json::Value = Figment::from(provider)
///     .extract()
///     .expect("expected extraction to succeed");
/// assert_eq!(value, serde_json::json!({}));
/// ```
///
/// # Errors
///
/// Returns an [`OrthoError`] if JSON serialization fails.
pub fn sanitized_provider<T: Serialize>(value: &T) -> OrthoResult<Serialized<serde_json::Value>> {
    sanitize_value(value).map(Serialized::defaults)
}
