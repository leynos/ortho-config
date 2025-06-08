//! Core crate for the `OrthoConfig` configuration framework.
//!
//! This crate defines the [`OrthoConfig`] trait and supporting error types. The
//! actual implementation of the derive macro lives in the companion
//! `ortho_config_macros` crate.

pub use ortho_config_macros::OrthoConfig;

mod error;
mod file;

pub use error::OrthoError;
pub use file::load_config_file;

/// Trait implemented for structs that represent application configuration.
pub trait OrthoConfig: Sized + serde::de::DeserializeOwned {
    /// Loads, merges, and deserializes configuration from all available
    /// sources according to predefined precedence rules.
    #[allow(clippy::result_large_err)]
    fn load() -> Result<Self, OrthoError>;
}
