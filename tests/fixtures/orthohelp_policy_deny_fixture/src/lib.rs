//! Policy deny-mode fixture for `cargo-orthohelp` integration tests.
//!
//! Provides a minimal flat configuration whose policy check exercises deny
//! mode with one malformed exception, per Decision D10.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Minimal flat configuration schema for the policy deny fixture.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "POLICY_FIXTURE")]
pub struct SimplePolicyConfig {
    /// Endpoint host used by the policy fixture.
    #[ortho_config(default = String::from("localhost"))]
    pub host: String,
}
