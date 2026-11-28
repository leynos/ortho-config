//! Fixtures for the profile-field disclosure boundary (roadmap 9.1.2).
//!
//! Every value-bearing slot here holds a literal that appears nowhere in the
//! public descriptions, so a generated artefact containing one is proof that
//! the projection failed rather than a coincidence of shared text.
//!
//! - [`RedactionFixtureConfig`] classifies fields as `secret` or
//!   `reference_only` at every level: ordinary, hidden from help, file-only,
//!   and one nested child command. Its root and sibling command examples repeat
//!   the child's sentinel, so the conservative command-example rule is
//!   observable.
//! - [`UnsupportedRedactionFixtureConfig`] classifies a field without opting
//!   into profiles. Projection still applies, but no profile declaration is
//!   invented.
//! - [`OrdinaryFixtureConfig`] declares no classification at all, so its
//!   defaults and examples must survive unchanged.

use clap::{Parser, Subcommand};
use ortho_config::{OrthoConfig, OrthoConfigSubcommandDocs};
use serde::{Deserialize, Serialize};

/// Profile-enabled command tree whose protected literals must never be published.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "REDACTION_FIXTURE",
    profiles,
    example(code = "redaction-fixture rotate --replacement NESTED-SECRET-DEFAULT-SENTINEL")
)]
pub struct RedactionFixtureConfig {
    /// Region identifier whose public default stays visible.
    #[ortho_config(default = String::from("eu-west"))]
    pub region: String,

    /// Public mode whose allowed values stay visible.
    #[ortho_config(default = String::from("fast"), value(type = "enum(fast, safe)"))]
    pub mode: String,

    /// Confidential token.
    #[arg(long, default_value = "SECRET-DEFAULT-SENTINEL")]
    #[ortho_config(
        profile_field = "secret",
        value(type = "enum(SECRET-ENUM-ALPHA, SECRET-ENUM-BETA)"),
        example(code = "redaction-fixture --token SECRET-EXAMPLE-SENTINEL")
    )]
    pub token: String,

    /// Locator the application resolves elsewhere.
    #[arg(long, default_value = "REFERENCE-DEFAULT-SENTINEL")]
    #[ortho_config(
        profile_field = "reference_only",
        value(type = "enum(REFERENCE-ENUM-ALPHA, REFERENCE-ENUM-BETA)"),
        example(code = "redaction-fixture --locator REFERENCE-EXAMPLE-SENTINEL")
    )]
    pub locator: String,

    /// Confidential key deliberately hidden from help.
    #[arg(long, default_value = "HIDDEN-DEFAULT-SENTINEL")]
    #[ortho_config(
        profile_field = "secret",
        cli(hide_in_help),
        example(code = "redaction-fixture --api-key HIDDEN-EXAMPLE-SENTINEL")
    )]
    pub api_key: String,

    /// Confidential default that clap would otherwise publish.
    ///
    /// `cli_default_as_absent` makes the derive read this clap literal as the
    /// field's documentation default before projection, unlike the fields
    /// above whose `default_value` never leaves clap. The projection is
    /// therefore the only thing standing between this literal and a published
    /// artefact.
    #[arg(long, default_value = "INFERRED-DEFAULT-SENTINEL")]
    #[ortho_config(
        profile_field = "secret",
        cli_default_as_absent,
        cli(hide_in_help),
        example(code = "redaction-fixture --inferred-secret INFERRED-EXAMPLE-SENTINEL")
    )]
    pub inferred_secret: String,

    /// Reference only configurable through a file.
    ///
    /// `#[arg(skip)]` keeps clap from treating the field as an operand;
    /// `skip_cli` alone only hides it from the generated loader parser.
    #[arg(skip)]
    #[ortho_config(
        skip_cli,
        profile_field = "reference_only",
        default = String::from("FILE-ONLY-DEFAULT-SENTINEL"),
        example(code = "credential_file = FILE-ONLY-EXAMPLE-SENTINEL"),
        help_id = "redaction-fixture-args-credential_file-help",
        long_help_id = "redaction-fixture-args-credential_file-long_help"
    )]
    pub credential_file: String,

    /// Selected redaction-fixture command.
    #[serde(skip)]
    #[command(subcommand)]
    pub command: RedactionFixtureCommand,
}

/// Top-level commands for [`RedactionFixtureConfig`].
#[derive(Debug, Clone, PartialEq, Eq, Subcommand, OrthoConfigSubcommandDocs)]
pub enum RedactionFixtureCommand {
    /// Rotates the confidential token.
    #[command(name = "rotate")]
    Rotate(RedactionRotateCommand),
    /// Reports fixture status.
    #[command(name = "status")]
    Status(RedactionStatusCommand),
}

impl Default for RedactionFixtureCommand {
    fn default() -> Self {
        Self::Rotate(RedactionRotateCommand::default())
    }
}

/// Options for [`RedactionFixtureCommand::Rotate`].
#[derive(Debug, Default, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "REDACTION_FIXTURE")]
pub struct RedactionRotateCommand {
    /// Confidential replacement token.
    #[arg(long, default_value = "NESTED-SECRET-DEFAULT-SENTINEL")]
    #[ortho_config(
        profile_field = "secret",
        value(type = "enum(NESTED-SECRET-ENUM-ALPHA, NESTED-SECRET-ENUM-BETA)"),
        example(code = "redaction-fixture rotate --replacement NESTED-SECRET-EXAMPLE-SENTINEL")
    )]
    pub replacement: String,

    /// Reports the rotation without applying it.
    #[ortho_config(default = false)]
    pub is_dry_run: bool,
}

/// Sibling command whose example repeats the protected child's sentinel.
#[derive(Debug, Default, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "REDACTION_FIXTURE",
    example(code = "redaction-fixture status NESTED-SECRET-DEFAULT-SENTINEL")
)]
pub struct RedactionStatusCommand {
    /// Public detail level whose default stays visible.
    #[ortho_config(default = String::from("summary"))]
    pub detail_level: String,
}

/// Command that classifies a field without opting into profiles.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "UNSUPPORTED_REDACTION_FIXTURE")]
pub struct UnsupportedRedactionFixtureConfig {
    /// Confidential token on a document with no profile declaration.
    #[arg(long, default_value = "UNSUPPORTED-DEFAULT-SENTINEL")]
    #[ortho_config(profile_field = "secret")]
    pub token: String,
}

/// Command with no classification, whose public metadata must survive intact.
#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(
    prefix = "ORDINARY_FIXTURE",
    example(code = "ordinary-fixture --name demo")
)]
pub struct OrdinaryFixtureConfig {
    /// Plain name whose default and examples stay visible.
    #[ortho_config(
        default = String::from("plain-default"),
        example(code = "ordinary-fixture --name plain-example")
    )]
    pub name: String,

    /// Verbose switch whose default stays visible.
    #[ortho_config(default = false)]
    pub is_verbose: bool,
}
