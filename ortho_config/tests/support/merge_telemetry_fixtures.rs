//! Configuration and vocabulary fixtures shared by the merge telemetry suites.
//!
//! Split from `merge_telemetry.rs` alongside its counter cases so the two
//! suites load the same shapes: a fixture that drifted would make the
//! counter expectations incomparable across the two files. The redaction
//! vocabulary lives with the assertion that enforces it.

use clap::Parser;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MERGE_TELEMETRY_")]
pub struct DerivedConfig {
    pub jobs: u16,
}

#[derive(Debug, Default, Deserialize, Parser, Serialize)]
#[command(name = "telemetry")]
pub struct TelemetrySubcommand {
    #[arg(long)]
    pub jobs: Option<u16>,
}

/// A profile-opted-in config, so the profile load boundary is instrumented.
///
/// The `jobs` default supplies the success case's value without needing a
/// file or an environment variable.
#[derive(Debug, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "MERGE_TELEMETRY_", profiles)]
pub struct ProfileTelemetryConfig {
    #[ortho_config(default = 3)]
    pub jobs: u16,
}
