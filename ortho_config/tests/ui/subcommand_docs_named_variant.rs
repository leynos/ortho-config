//! Test case exercising named-field subcommand docs variants.

use ortho_config::OrthoConfigSubcommandDocs;

struct Args;

#[derive(OrthoConfigSubcommandDocs)]
enum Commands {
    Run { args: Args },
}

fn main() {}
