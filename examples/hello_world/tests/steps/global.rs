//! Step definitions for the `hello_world` example.
//! Drive the binary and assert its outputs.
use crate::World;
use cucumber::{then, when};

/// Runs the binary without additional arguments.
#[when("I run the hello world example")]
pub async fn run_without_args(world: &mut World) {
    world.run_hello(None).await;
}

#[when(expr = "I run the hello world example with arguments {string}")]
// Step captures arrive as owned `String` values from cucumber; move them
// into the world helper to avoid cloning.
pub async fn run_with_args(world: &mut World, args: String) {
    world.run_hello(Some(args)).await;
}

#[then("the command succeeds")]
pub fn command_succeeds(world: &mut World) {
    world.assert_success();
}

#[then("the command fails")]
pub fn command_fails(world: &mut World) {
    world.assert_failure();
}

#[then(expr = "stdout contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
// Step captures arrive as owned `String` values from cucumber.
pub fn stdout_contains(world: &mut World, expected: String) {
    world.assert_stdout_contains(&expected);
}

#[then(expr = "stderr contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
// Step captures arrive as owned `String` values from cucumber.
pub fn stderr_contains(world: &mut World, expected: String) {
    world.assert_stderr_contains(&expected);
}
