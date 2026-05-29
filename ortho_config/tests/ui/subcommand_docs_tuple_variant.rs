//! Test case exercising multi-field subcommand docs tuple variants.

use ortho_config::OrthoConfigSubcommandDocs;

struct Args;

#[derive(OrthoConfigSubcommandDocs)]
enum Commands {
    Run(Args, Args),
}

fn main() {}
