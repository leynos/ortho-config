//! Command-line interface definitions for `cargo-orthohelp`.

use camino::Utf8PathBuf;
use clap::{Args as ClapArgs, Parser, ValueEnum};

/// Output formats supported by `cargo-orthohelp`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Emit the localized IR JSON output.
    Ir,
    /// Emit Unix roff man pages (not yet implemented).
    Man,
    /// Emit `PowerShell` help (not yet implemented).
    Ps,
    /// Emit all outputs (not yet implemented).
    All,
}

impl OutputFormat {
    /// Returns the CLI-friendly string for this output format.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ir => "ir",
            Self::Man => "man",
            Self::Ps => "ps",
            Self::All => "all",
        }
    }
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
    /// Output format selection (IR is the only supported format for now).
    #[arg(long, value_enum, default_value_t = OutputFormat::Ir)]
    pub format: OutputFormat,
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
