//! Command-line interface definitions for `cargo-orthohelp`.

use camino::Utf8PathBuf;
use clap::{ArgAction, Args as ClapArgs, Parser, ValueEnum};

/// Output formats supported by `cargo-orthohelp`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Emit the localized IR JSON output.
    Ir,
    /// Emit Unix roff man pages.
    Man,
    /// Emit `PowerShell` help output.
    Ps,
    /// Emit all outputs (IR, man pages, and `PowerShell` help).
    All,
}

/// Parsed CLI arguments for `cargo-orthohelp`.
#[derive(Debug, Parser)]
#[command(name = "cargo-orthohelp")]
#[command(about = "Generate localized OrthoConfig documentation IR")]
#[command(version)]
pub struct Args {
    /// Cargo package to document.
    #[arg(long)]
    pub package: Option<String>,
    /// Binary target name (used for metadata validation).
    #[arg(long)]
    pub bin: Option<String>,
    /// Select the package's library target.
    #[arg(long = "lib")]
    pub is_lib: bool,
    /// Root configuration type (for example, `my_crate::Config`).
    #[arg(long, value_name = "path::Type")]
    pub root_type: Option<String>,
    /// Locale to render (repeat for multiple locales).
    #[arg(long, value_name = "locale")]
    pub locale: Vec<String>,
    /// Generate for every locale declared in package metadata.
    #[arg(long = "all-locales")]
    pub should_use_all_locales: bool,
    /// Output directory for generated artefacts.
    #[arg(long, value_name = "path")]
    pub out_dir: Option<Utf8PathBuf>,
    /// Bridge cache behaviour flags.
    #[command(flatten)]
    pub cache: CacheArgs,
    /// Output format selection.
    #[arg(long, value_enum, default_value_t = OutputFormat::Ir)]
    pub format: OutputFormat,
    /// Man page generation arguments.
    #[command(flatten)]
    pub man: ManArgs,
    /// `PowerShell` generation arguments.
    #[command(flatten)]
    pub powershell: PowerShellArgs,
}

/// Bridge cache behaviour flags.
#[derive(Debug, ClapArgs, Clone, Copy)]
pub struct CacheArgs {
    /// Cache and reuse the bridge IR when possible.
    #[arg(long = "cache")]
    pub should_cache: bool,
    /// Skip building the bridge and rely on cached IR.
    #[arg(long = "no-build")]
    pub should_skip_build: bool,
}

/// Man page generation arguments.
#[derive(Debug, ClapArgs, Clone)]
pub struct ManArgs {
    /// Man page section number (1-8, default: 1 for user commands).
    #[arg(
        long = "man-section",
        value_name = "N",
        default_value = "1",
        value_parser = clap::value_parser!(u8).range(1..=8)
    )]
    pub section: u8,
    /// Date for man page header (format: YYYY-MM-DD or "January 2026").
    #[arg(long = "man-date", value_name = "DATE")]
    pub date: Option<String>,
    /// Generate separate man pages for each subcommand.
    #[arg(long = "man-split-subcommands")]
    pub should_split_subcommands: bool,
}

/// `PowerShell` help generation arguments.
#[derive(Debug, ClapArgs, Clone)]
pub struct PowerShellArgs {
    /// `PowerShell` module name override.
    #[arg(long = "ps-module-name", value_name = "NAME")]
    pub module_name: Option<String>,
    /// Split subcommands into separate wrapper functions.
    #[arg(long = "ps-split-subcommands", value_name = "BOOL", action = ArgAction::Set)]
    pub split_subcommands: Option<bool>,
    /// Include `CommonParameters` in help output.
    #[arg(
        long = "ps-include-common-parameters",
        value_name = "BOOL",
        action = ArgAction::Set
    )]
    pub include_common_parameters: Option<bool>,
    /// `HelpInfoUri` for Update-Help payloads.
    #[arg(long = "ps-help-info-uri", value_name = "URI")]
    pub help_info_uri: Option<String>,
    /// Ensure an en-US help file exists.
    #[arg(
        long = "ensure-en-us",
        value_name = "BOOL",
        default_value_t = true,
        action = ArgAction::Set
    )]
    pub ensure_en_us: bool,
}
