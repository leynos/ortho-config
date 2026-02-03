//! Fixture crate for `cargo-orthohelp` integration tests.
//!
//! Provides a comprehensive configuration struct for testing man page generation
//! with various field types, environment variables, file keys, and enums.

use clap::ValueEnum;
use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

/// Log level for the fixture service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize, ValueEnum)]
pub enum LogLevel {
    /// Debug logging.
    Debug,
    /// Informational logging (default).
    #[default]
    Info,
    /// Warning logging.
    Warn,
    /// Error logging only.
    Error,
}

/// Configuration schema for IR and man page generation tests.
///
/// This struct exercises various `OrthoConfig` features:
/// - CLI flags with short and long forms
/// - Environment variable mappings
/// - File configuration keys
/// - Enum types with possible values
/// - Default values
/// - Required and optional fields
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "FIXTURE")]
pub struct FixtureConfig {
    /// Port used by the fixture service.
    #[ortho_config(default = 8080, cli_short = 'p')]
    pub port: u16,

    /// Hostname to bind the service to.
    #[ortho_config(default = String::from("localhost"))]
    pub host: String,

    /// Log level for the service.
    #[ortho_config(default = LogLevel::Info)]
    pub log_level: LogLevel,

    /// Enable verbose output.
    #[ortho_config(default = false, cli_short = 'v')]
    pub is_verbose: bool,

    /// Configuration file path.
    pub config_path: Option<String>,

    /// Number of worker threads.
    #[ortho_config(default = 4)]
    pub workers: u32,

    /// Request timeout in seconds.
    #[ortho_config(default = 30)]
    pub timeout: u64,
}
