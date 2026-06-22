//! Compile-time contract proving `LocalizedParse` is available to `clap::Parser`
//! types.

use clap::Parser;
use ortho_config::{LocalizedParse, NoOpLocalizer};

#[derive(Debug, Parser)]
#[command(name = "demo", bin_name = "demo")]
struct Cli {
    #[arg(long)]
    verbose: bool,
}

fn main() -> Result<(), clap::Error> {
    let localizer = NoOpLocalizer::new();
    let cli = Cli::try_parse_localized_from(["demo", "--verbose"], &localizer)?;
    assert!(cli.verbose);
    Ok(())
}
