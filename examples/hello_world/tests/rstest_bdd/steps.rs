//! Step definitions driving the hello_world rstest-bdd canary scenario.

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
) {
    let args = vec![
        hello_world_binary.to_string(),
        "--recipient".to_string(),
        recipient,
    ];
    let cli =
        HelloWorldCli::load_from_iter(args).expect("hello_world configuration should load");
    hello_world_state.cli.set(cli);
}

#[then("the recipient name resolves to {expected}")]
fn assert_recipient(hello_world_state: &HelloWorldState, expected: String) {
    let actual = hello_world_state
        .cli
        .with_ref(|cli| cli.recipient.clone())
        .expect("the CLI must be loaded before checking the recipient");
    assert_eq!(actual, expected);
}
