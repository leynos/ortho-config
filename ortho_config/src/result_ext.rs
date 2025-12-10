//! Extensions for mapping errors to `OrthoResult` concisely.
//!
//! These helpers reduce repetitive `.map_err(|e| OrthoError::…(e).into())`
//! patterns when converting external error types into the crate’s
//! `OrthoResult<T>` alias (`Result<T, Arc<OrthoError>>`).
//!
//! - Use [`OrthoResultExt::into_ortho`] for error types that implement
//!   `Into<OrthoError>` (e.g., `serde_json::Error`).
//! - Use [`OrthoMergeExt::into_ortho_merge`] specifically for
//!   `figment::Error` cases that should become `OrthoError::Merge`.
//!
//! # Examples
//!
//! ```
//! use ortho_config::{OrthoResult, OrthoResultExt};
//!
//! fn serialize() -> OrthoResult<serde_json::Value> {
//!     // serde_json::Error implements Into<OrthoError>
//!     serde_json::to_value(&42).into_ortho()
//! }
//! ```
//!
//! ```ignore
//! use ortho_config::OrthoMergeExt;
//! # use figment::{Figment, providers::Toml};
//! # let fig = Figment::from(Toml::string("key = 1"));
//! let result: Result<(), figment::Error> = fig.extract();
//! let merged = result.into_ortho_merge();
//! ```

use crate::{OrthoError, OrthoResult};
use std::sync::Arc;

/// Generic extension for mapping any `Result<T, E>` with `E: Into<OrthoError>`
/// into an `OrthoResult<T>`.
pub trait OrthoResultExt<T, E> {
    /// Convert `Result<T, E>` into `OrthoResult<T>` using `Into<OrthoError>`.
    ///
    /// # Errors
    ///
    /// Propagates the original error after conversion into `Arc<OrthoError>`.
    fn into_ortho(self) -> OrthoResult<T>;
}

impl<T, E> OrthoResultExt<T, E> for Result<T, E>
where
    E: Into<OrthoError>,
{
    fn into_ortho(self) -> OrthoResult<T> {
        self.map_err(|e| Arc::new(e.into()))
    }
}

/// Extension tailored to mapping `figment::Error` into a merge failure.
pub trait OrthoMergeExt<T> {
    /// Convert `Result<T, figment::Error>` into `OrthoResult<T>` as a
    /// [`OrthoError::Merge`].
    ///
    /// # Errors
    ///
    /// Returns an `OrthoError::Merge` wrapped in `Arc` when the input is `Err`.
    fn into_ortho_merge(self) -> OrthoResult<T>;
}

impl<T> OrthoMergeExt<T> for Result<T, figment::Error> {
    fn into_ortho_merge(self) -> OrthoResult<T> {
        self.map_err(|e| Arc::new(OrthoError::merge(e)))
    }
}

/// Extension tailored to mapping `serde_json::Error` into a merge failure.
///
/// Use this trait when deserializing JSON values in a merge context where
/// failures should be attributed to the merge phase rather than the gathering
/// phase.
///
/// # Examples
///
/// ```rust
/// use ortho_config::{OrthoJsonMergeExt, OrthoError};
///
/// let bad_json: Result<i32, serde_json::Error> = serde_json::from_str("\"not_a_number\"");
/// let result = bad_json.into_ortho_merge_json();
/// assert!(matches!(&*result.unwrap_err(), OrthoError::Merge { .. }));
/// ```
#[cfg(feature = "serde_json")]
pub trait OrthoJsonMergeExt<T> {
    /// Convert `Result<T, serde_json::Error>` into `OrthoResult<T>` as an
    /// [`OrthoError::Merge`].
    ///
    /// # Errors
    ///
    /// Returns an `OrthoError::Merge` wrapped in `Arc` when the input is `Err`.
    fn into_ortho_merge_json(self) -> OrthoResult<T>;
}

#[cfg(feature = "serde_json")]
impl<T> OrthoJsonMergeExt<T> for Result<T, serde_json::Error> {
    fn into_ortho_merge_json(self) -> OrthoResult<T> {
        self.map_err(|e| {
            // Preserve structured error information via figment's Kind::Message.
            // serde_json::Error's Display includes the error category (syntax, data, io),
            // the specific message, and location details when available.
            let figment_err = figment::Error::from(e.to_string());
            Arc::new(OrthoError::merge(figment_err))
        })
    }
}

/// Convert shared Ortho errors into `figment::Error` for interop in tests and
/// integrations that expect Figment's error type.
pub trait IntoFigmentError {
    /// Convert into a `figment::Error`, preserving message text. For
    /// `Merge/Gathering` variants, structured details may be lost due to shared
    /// ownership; consumers should prefer `OrthoError` where possible.
    fn into_figment(self) -> figment::Error;
}

fn clone_figment(shared: &Arc<OrthoError>) -> figment::Error {
    match shared.as_ref() {
        OrthoError::Merge { source } | OrthoError::Gathering(source) => source.as_ref().clone(),
        other => figment::Error::from(other.to_string()),
    }
}

impl IntoFigmentError for Arc<OrthoError> {
    fn into_figment(self) -> figment::Error {
        // Prefer preserving structured Figment details when we can take
        // ownership of the error. This retains kind, metadata and sources for
        // Merge/Gathering variants via `From<OrthoError> for figment::Error`.
        match Self::try_unwrap(self) {
            Ok(err) => err.into(),
            Err(shared) => clone_figment(&shared),
        }
    }
}

impl IntoFigmentError for &Arc<OrthoError> {
    fn into_figment(self) -> figment::Error {
        clone_figment(self)
    }
}

/// Extension to convert `Result<T, Arc<OrthoError>>` into `Result<T, figment::Error>`.
#[expect(
    clippy::result_large_err,
    reason = "figment::Error is inherently large and this adapter serves only tests."
)]
pub trait ResultIntoFigment<T> {
    /// Map the `Arc<OrthoError>` error into a `figment::Error` using
    /// [`IntoFigmentError`].
    ///
    /// # Errors
    ///
    /// Returns a `figment::Error` containing the original message.
    fn to_figment(self) -> Result<T, figment::Error>;
}

impl<T> ResultIntoFigment<T> for Result<T, Arc<OrthoError>> {
    fn to_figment(self) -> Result<T, figment::Error> {
        self.map_err(IntoFigmentError::into_figment)
    }
}

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn into_ortho_merge_json_produces_merge_error() {
        let bad_json: Result<i32, serde_json::Error> = serde_json::from_str("\"not_a_number\"");
        let ortho_result = bad_json.into_ortho_merge_json();
        let err = ortho_result.expect_err("expected error from invalid JSON");

        assert!(
            matches!(&*err, OrthoError::Merge { .. }),
            "expected Merge error variant, got {err:?}"
        );
    }

    #[rstest]
    fn into_ortho_merge_json_preserves_ok_value() {
        let ok_result: Result<i32, serde_json::Error> = Ok(42);
        let ortho_result = ok_result.into_ortho_merge_json();

        assert_eq!(ortho_result.expect("expected Ok"), 42);
    }

    #[rstest]
    fn into_ortho_merge_json_error_message_contains_location() {
        let bad_json: Result<i32, serde_json::Error> = serde_json::from_str("\"bad\"");
        let ortho_result = bad_json.into_ortho_merge_json();
        let err = ortho_result.expect_err("expected error");

        let message = err.to_string();
        assert!(
            message.contains("line") && message.contains("column"),
            "error message should contain line and column: {message}"
        );
    }
}
