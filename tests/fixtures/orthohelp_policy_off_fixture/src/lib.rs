//! Policy off-mode fixture for `cargo-orthohelp` integration tests.
//!
//! Provides a minimal flat configuration whose policy check exercises off
//! mode even though the exception list contains malformed, redundant, and
//! duplicate entries, per Decision D9/D10.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Minimal flat configuration schema for the policy off fixture.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "POLICY_FIXTURE")]
pub struct SimplePolicyConfig {
    /// Endpoint host used by the policy fixture.
    #[ortho_config(default = String::from("localhost"))]
    pub host: String,
}
