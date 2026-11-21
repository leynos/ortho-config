//! A subcommand selector is not a configuration subtree, so `nested` on that
//! field must be rejected instead of being ignored.

use ortho_config::OrthoConfig;

/// The root tries to compose the selector's schema.
#[derive(clap::Parser, OrthoConfig)]
#[ortho_config(prefix = "UI2_")]
struct Cli {
    /// A selector cannot also be a nested configuration object.
    #[command(subcommand)]
    #[ortho_config(nested)]
    command: Option<Commands>,
}

/// The selector only chooses a variant; it has no requirement schema.
#[derive(clap::Subcommand)]
enum Commands {
    Run,
}

fn main() {}