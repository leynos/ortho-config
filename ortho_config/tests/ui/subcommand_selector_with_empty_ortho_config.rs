//! UI test: verify `#[command(subcommand)]` fields reject `#[ortho_config()]`.
//!
//! The derive should emit a compile-time error directing users to remove the
//! conflicting attribute.

use ortho_config::OrthoConfig;

#[derive(clap::Parser, OrthoConfig)]
struct Cli {
    #[command(subcommand)]
    #[ortho_config()]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    Run,
}

fn main() {}
