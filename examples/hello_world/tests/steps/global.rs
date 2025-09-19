use crate::World;
use cucumber::{then, when};
use shlex::split;

/// Runs the binary without additional arguments.
#[when("I run the hello world example")]
pub async fn run_without_args(world: &mut World) {
    world.run_example(Vec::new()).await;
}

#[when(expr = "I run the hello world example with arguments {string}")]
pub async fn run_with_args(world: &mut World, args: String) {
    let parsed = if args.trim().is_empty() {
        Vec::new()
    } else {
        split(&args).expect("parse CLI arguments")
    };
    world.run_example(parsed).await;
}

#[then("the command succeeds")]
pub fn command_succeeds(world: &mut World) {
    let result = world.result();
    assert!(
        result.success,
        "expected success, stderr was: {}",
        result.stderr
    );
}

#[then("the command fails")]
pub fn command_fails(world: &mut World) {
    let result = world.result();
    assert!(
        !result.success,
        "expected failure, stdout was: {}",
        result.stdout
    );
}

#[then(expr = "stdout contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub fn stdout_contains(world: &mut World, expected: String) {
    let result = world.result();
    assert!(
        result.stdout.contains(&expected),
        "stdout did not contain {expected:?}. stdout was: {:?}",
        result.stdout
    );
}

#[then(expr = "stderr contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub fn stderr_contains(world: &mut World, expected: String) {
    let result = world.result();
    assert!(
        result.stderr.contains(&expected),
        "stderr did not contain {expected:?}. stderr was: {:?}",
        result.stderr
    );
}
