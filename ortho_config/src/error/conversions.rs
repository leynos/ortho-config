//! Trait-based conversions between external error types and `OrthoError`.

use figment::Error as FigmentError;

use super::OrthoError;

/// Convert JSON encoding or decoding failures into
/// [`OrthoError::Gathering`].
#[cfg(feature = "serde_json")]
impl From<serde_json::Error> for OrthoError {
    fn from(e: serde_json::Error) -> Self {
        Self::Gathering(Box::new(figment::Error::from(format!(
            "JSON error: {} at line {}, column {}",
            e,
            e.line(),
            e.column()
        ))))
    }
}

impl From<clap::Error> for OrthoError {
    fn from(e: clap::Error) -> Self {
        Self::CliParsing(e.into())
    }
}

impl From<FigmentError> for OrthoError {
    fn from(e: FigmentError) -> Self {
        Self::Gathering(e.into())
    }
}

impl From<OrthoError> for FigmentError {
    /// Allow using `?` in tests and examples that return `figment::Error`.
    fn from(e: OrthoError) -> Self {
        match e {
            // Preserve the original Figment error (keeps kind, metadata, and sources).
            OrthoError::Merge { source: fe } | OrthoError::Gathering(fe) => *fe,
            // Fall back to a message for other variants.
            other => Self::from(other.to_string()),
        }
    }
}
