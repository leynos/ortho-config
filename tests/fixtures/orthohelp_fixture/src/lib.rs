//! Fixture crate for `cargo-orthohelp` integration tests.

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Minimal configuration schema for IR generation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "FIXTURE")]
pub struct FixtureConfig {
    /// Port used by the demo service.
    #[ortho_config(default = 8080)]
    pub port: u16,
}
