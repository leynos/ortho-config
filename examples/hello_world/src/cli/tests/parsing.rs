//! CLI argument parsing scenarios.

use super::helpers::*;
use anyhow::Result;
use rstest::rstest;

#[rstest]
#[case::greet(
    &[
        "--recipient",
        "Crew",
        "-s",
        "Hi",
        "greet",
        "--preamble",
        "Good morning",
        "--punctuation",
        "?!",
    ],
    &assert_greet_command,
)]
#[case::take_leave(
    &[
        "--is-excited",
        "take-leave",
        "--parting",
        "Cheerio",
        "--gift",
        "flowers",
        "--remind-in",
        "20",
        "--channel",
        "message",
        "--wave",
    ],
    &assert_take_leave_command,
)]
fn command_line_parses_expected_variants(
    #[case] args: &[&str],
    #[case] assert_cli: CommandAssertion<'_>,
) -> Result<()> {
    let cli = parse_command_line(args)?;
    assert_cli(cli)?;
    Ok(())
}
