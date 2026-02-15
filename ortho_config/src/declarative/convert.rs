//! JSON conversion helpers used by declarative merging.

use serde_json::Value;

use crate::{OrthoResult, result_ext::OrthoJsonMergeExt, result_ext::OrthoResultExt};

/// Deserialise a JSON [`Value`] into `T`.
///
/// # Errors
///
/// Returns an [`crate::OrthoError`] when deserialisation fails.
///
/// # Examples
///
/// ```rust
/// use ortho_config::declarative::from_value;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct App { port: u16 }
///
/// let v = json!({"port": 8080});
/// let app: App = from_value(v).expect("value deserialises");
/// assert_eq!(app.port, 8080);
/// ```
pub fn from_value<T: serde::de::DeserializeOwned>(value: Value) -> OrthoResult<T> {
    serde_json::from_value(value).into_ortho()
}

/// Deserialise a JSON [`Value`] into `T`, routing errors to [`crate::OrthoError::Merge`].
///
/// Use this function when deserialising in a merge context where failures should
/// be attributed to the merge phase rather than the gathering phase. This
/// semantic distinction clarifies that failures at the merge phase (combining
/// and deserialising) are separate from failures during the gathering phase
/// (reading sources).
///
/// # Errors
///
/// Returns an [`crate::OrthoError::Merge`] when deserialisation fails.
///
/// # Examples
///
/// ```rust
/// use ortho_config::declarative::from_value_merge;
/// use ortho_config::OrthoError;
/// use serde::Deserialize;
/// use serde_json::json;
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct App { port: u16 }
///
/// // Valid input deserialises successfully.
/// let v = json!({"port": 8080});
/// let app: App = from_value_merge(v).expect("value deserialises successfully");
/// assert_eq!(app.port, 8080);
///
/// // Invalid input produces Merge error.
/// let invalid = json!({"port": "not_a_number"});
/// let err = from_value_merge::<App>(invalid).unwrap_err();
/// assert!(matches!(&*err, OrthoError::Merge { .. }));
/// ```
pub fn from_value_merge<T: serde::de::DeserializeOwned>(value: Value) -> OrthoResult<T> {
    serde_json::from_value(value).into_ortho_merge_json()
}
