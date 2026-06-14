//! Fixture crate for `cargo-orthohelp` integration tests.
//!
//! Provides a comprehensive configuration struct for testing man page generation
//! with various field types, environment variables, file keys, and enums.

use clap::{Parser, Subcommand, ValueEnum};
use ortho_config::{OrthoConfig, OrthoConfigSubcommandDocs};
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
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "FIXTURE",
    windows(
        module_name = "FixtureHelp",
        export_aliases = ["fixture-help"],
        include_common_parameters = true,
        split_subcommands = false
    )
)]
pub struct FixtureConfig {
    /// Port used by the fixture service.
    #[ortho_config(default = 8080, cli_short = 'p', file(key_path = "server.port"))]
    pub port: u16,

    /// Hostname to bind the service to.
    #[ortho_config(default = String::from("localhost"), env(name = "FIXTURE_HOST"))]
    pub host: String,

    /// Log level for the service.
    #[ortho_config(default = LogLevel::Info, value(type = "enum(Debug, Info, Warn, Error)"))]
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

    /// Enables the legacy processing mode.
    #[ortho_config(
        default = false,
        deprecated(note_id = "fixture.fields.is_legacy_mode.deprecated")
    )]
    pub is_legacy_mode: bool,
}

/// Nested command-tree fixture for end-to-end bridge smoke tests.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "NESTED_FIXTURE",
    windows(
        module_name = "NestedFixture",
        include_common_parameters = true,
        split_subcommands = true
    )
)]
pub struct NestedFixtureConfig {
    /// Global configuration value shared by nested fixture commands.
    #[ortho_config(default = String::from("workspace"))]
    pub global: String,

    /// Selected nested fixture command.
    #[serde(skip)]
    #[command(subcommand)]
    pub command: NestedFixtureCommand,
}

impl Default for NestedFixtureConfig {
    fn default() -> Self {
        Self {
            global: String::from("workspace"),
            command: NestedFixtureCommand::default(),
        }
    }
}

/// Top-level commands for [`NestedFixtureConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Subcommand, OrthoConfigSubcommandDocs)]
pub enum NestedFixtureCommand {
    /// Greets a named recipient.
    #[command(name = "greet")]
    Greet(NestedGreetCommand),
    /// Prints version information.
    #[command(name = "version")]
    Version(NestedVersionCommand),
    /// Administers fixture state.
    #[command(name = "admin")]
    Admin(NestedAdminCommand),
}

impl Default for NestedFixtureCommand {
    fn default() -> Self {
        Self::Greet(NestedGreetCommand::default())
    }
}

/// Options for the nested fixture greet command.
#[derive(Debug, Default, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "NESTED_FIXTURE")]
pub struct NestedGreetCommand {
    /// Recipient to greet.
    #[ortho_config(default = String::from("World"))]
    pub recipient: String,
    /// Adds an exclamation mark to the greeting.
    #[ortho_config(default = false)]
    pub is_excited: bool,
}
/// Empty command proving leaf commands without options survive the bridge.
#[derive(Debug, Default, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "NESTED_FIXTURE")]
pub struct NestedVersionCommand {}

/// Options for the nested fixture admin command.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "NESTED_FIXTURE",
    windows(
        module_name = "NestedFixtureAdmin",
        include_common_parameters = false,
        split_subcommands = true
    )
)]
pub struct NestedAdminCommand {
    /// Scope to administer.
    #[ortho_config(default = String::from("local"))]
    pub scope: String,

    /// Selected admin operation.
    #[serde(skip)]
    #[command(subcommand)]
    pub command: NestedAdminSubcommand,
}

impl Default for NestedAdminCommand {
    fn default() -> Self {
        Self {
            scope: String::from("local"),
            command: NestedAdminSubcommand::default(),
        }
    }
}

/// Nested admin operations for [`NestedAdminCommand`].
#[derive(Debug, Clone, PartialEq, Eq, Subcommand, OrthoConfigSubcommandDocs)]
pub enum NestedAdminSubcommand {
    /// Audits fixture state.
    #[command(name = "audit")]
    Audit(NestedAuditCommand),
    /// Grants access to a principal.
    #[command(name = "grant-access")]
    GrantAccess(NestedGrantAccessCommand),
}

impl Default for NestedAdminSubcommand {
    fn default() -> Self {
        Self::Audit(NestedAuditCommand::default())
    }
}

/// Options for the nested fixture audit command.
#[derive(Debug, Default, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "NESTED_FIXTURE")]
pub struct NestedAuditCommand {
    /// Reports intended audit actions without applying them.
    #[ortho_config(default = false)]
    pub is_dry_run: bool,
}

/// Options for the nested fixture grant-access command.
#[derive(Debug, Default, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "NESTED_FIXTURE")]
pub struct NestedGrantAccessCommand {
    /// Principal receiving access.
    pub principal: Option<String>,
}
