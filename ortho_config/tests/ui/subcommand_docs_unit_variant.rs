//! Test case exercising unit subcommand docs variants.

use ortho_config::OrthoConfigSubcommandDocs;

#[derive(OrthoConfigSubcommandDocs)]
enum Commands {
    Run,
}

fn main() {}
