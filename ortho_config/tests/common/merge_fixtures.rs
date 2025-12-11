//! Shared fixtures for merge error routing tests.
//!
//! These structs are used across multiple test modules to verify merge error
//! routing behaviour. Centralising them here prevents divergence and reduces
//! maintenance burden.

use ortho_config::OrthoConfig;
use serde::Deserialize;

/// Minimal struct used by merge error routing tests.
///
/// Used to verify that deserialization errors during the merge phase produce
/// `OrthoError::Merge` rather than `OrthoError::Gathering`.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "TEST")]
pub struct MergeErrorSample {
    #[ortho_config(default = 8080)]
    pub port: u16,
}

/// Minimal struct used by vector append merge error routing tests.
///
/// Used to verify that append strategy deserialization errors during the merge
/// phase produce `OrthoError::Merge`.
#[derive(Debug, Deserialize, OrthoConfig)]
#[ortho_config(prefix = "TEST")]
pub struct VecAppendSample {
    #[ortho_config(merge_strategy = "append")]
    pub items: Vec<u32>,
}
