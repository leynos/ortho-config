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
// Clippy 1.81 does not emit `needless_pass_by_value`; keep the expectation
// for documentation consistency.
#[allow(unfulfilled_lint_expectations)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub async fn run_with_args(world: &mut World, args: String) {
    world.run_hello(Some(args.as_str())).await;
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
// Clippy 1.81 does not emit `needless_pass_by_value`; keep the expectation
// for documentation consistency.
#[allow(unfulfilled_lint_expectations)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub fn stdout_contains(world: &mut World, expected: String) {
    world.assert_stdout_contains(&expected);
}

#[then(expr = "stderr contains {string}")]
// Clippy 1.81 does not emit `needless_pass_by_value`; keep the expectation
// for documentation consistency.
#[allow(unfulfilled_lint_expectations)]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub fn stderr_contains(world: &mut World, expected: String) {
    world.assert_stderr_contains(&expected);
}
