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

/// Convert shared Ortho errors into `figment::Error` for interop in tests and
/// integrations that expect Figment's error type.
pub trait IntoFigmentError {
    /// Convert into a `figment::Error`, preserving message text. For
    /// `Merge/Gathering` variants, structured details may be lost due to shared
    /// ownership; consumers should prefer `OrthoError` where possible.
    fn into_figment(self) -> figment::Error;
}

impl IntoFigmentError for Arc<OrthoError> {
    fn into_figment(self) -> figment::Error {
        figment::Error::from(self.to_string())
    }
}
