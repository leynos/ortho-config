//! Converting `OrthoResult` values into `anyhow::Result` for test bodies.
//!
//! Integration tests report failures through `anyhow`, while the crate returns
//! `OrthoResult<T>` — that is, `Result<T, Arc<OrthoError>>`. Every suite needs
//! the same bridge, and before this module each grew its own: a free function
//! here, an inherent extension trait there, and a scattering of inline
//! `map_err(|err| anyhow!(err))` calls. They agreed on behaviour by accident
//! rather than by construction, so this module is the single conversion point.

use ortho_config::OrthoResult;

/// Converts an [`OrthoResult`] into an [`anyhow::Result`].
pub trait ToAnyhow<T> {
    /// Convert `self`, preserving the original error as the `anyhow` source.
    ///
    /// # Errors
    ///
    /// Propagates the original `OrthoError` wrapped in an [`anyhow::Error`].
    fn to_anyhow(self) -> anyhow::Result<T>;
}

impl<T> ToAnyhow<T> for OrthoResult<T> {
    fn to_anyhow(self) -> anyhow::Result<T> {
        self.map_err(|err| anyhow::anyhow!(err))
    }
}
