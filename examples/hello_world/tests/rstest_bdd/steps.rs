//! Step definitions driving the `hello_world` rstest-bdd canary scenario.

use anyhow::{Result, anyhow, ensure};
use super::fixtures::HelloWorldState;
use hello_world::cli::HelloWorldCli;
use rstest_bdd::ScenarioState as _;
use rstest_bdd_macros::{given, then, when};

#[given("the hello world scenario state is reset")]
fn reset_state(hello_world_state: &HelloWorldState) {
    hello_world_state.reset();
    assert!(
        hello_world_state.cli.is_empty(),
        "resetting the state must clear any previous CLI value"
    );
}

#[when("I load the hello world CLI with recipient {recipient}")]
fn load_cli(
    hello_world_state: &HelloWorldState,
    hello_world_binary: &str,
    recipient: String,
) -> Result<()> {
    let args = vec![
        hello_world_binary.to_owned(),
        "--recipient".to_owned(),
        recipient,
    ];
    let cli = HelloWorldCli::load_from_iter(args).map_err(anyhow::Error::from)?;
    hello_world_state.cli.set(cli);
    Ok(())
}

#[then("the recipient name resolves to {expected}")]
fn assert_recipient(hello_world_state: &HelloWorldState, expected: String) -> Result<()> {
    let actual = hello_world_state
        .cli
        .with_ref(|cli| cli.recipient.clone())
        .ok_or_else(|| anyhow!("the CLI must be loaded before checking the recipient"))?;
    ensure!(actual == expected, "unexpected recipient {actual:?}");
    Ok(())
}
