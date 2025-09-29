#![allow(
    unfulfilled_lint_expectations,
    reason = "Clippy 1.81 does not emit needless_pass_by_value for cucumber steps yet; expectations document the signature requirements."
)]
//! Step definitions for the `hello_world` example.
//! Drive the binary and assert its outputs.
use crate::{SampleConfigError, World};
use cucumber::gherkin::Step as GherkinStep;
use cucumber::{given, then, when};

/// Runs the binary without additional arguments.
#[when("I run the hello world example")]
pub async fn run_without_args(world: &mut World) {
    world.run_hello(None).await;
}

#[when(expr = "I run the hello world example with arguments {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
// Step captures arrive as owned `String` values from cucumber; lend them
// to the world helper for tokenisation.
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
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
// Step captures arrive as owned `String` values from cucumber; borrow them
// for assertions so the captured text remains available.
pub fn stdout_contains(world: &mut World, expected: String) {
    world.assert_stdout_contains(&expected);
}

#[then(expr = "stderr contains {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
// Step captures arrive as owned `String` values from cucumber; borrow them
// for assertions so the captured text remains available.
pub fn stderr_contains(world: &mut World, expected: String) {
    world.assert_stderr_contains(&expected);
}

#[given(expr = "the environment contains {string} = {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub fn environment_contains(world: &mut World, key: String, value: String) {
    world.set_env(key, value);
}

#[given(expr = "the environment does not contain {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub fn environment_does_not_contain(world: &mut World, key: String) {
    world.remove_env(&key);
}

#[given("the hello world config file contains:")]
pub fn config_file(world: &mut World, step: &GherkinStep) {
    let contents = step
        .docstring()
        .expect("config docstring provided for hello world example");
    world.write_config(contents);
}

/// Initialises the scenario using a repository sample configuration.
#[given(expr = "I start from the sample hello world config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub fn start_from_sample_config(world: &mut World, sample: String) {
    world.write_sample_config(&sample);
}

#[given(expr = "I start from a missing or invalid sample config {string}")]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Cucumber step signature requires owned String"
)]
pub fn start_from_invalid_sample_config(world: &mut World, sample: String) {
    match world.try_write_sample_config(&sample) {
        Ok(()) => panic!("expected sample config {sample:?} to be missing or invalid"),
        Err(
            SampleConfigError::OpenSample { .. }
            | SampleConfigError::ReadSample { .. }
            | SampleConfigError::WriteSample { .. },
        ) => {}
        Err(err) => panic!("unexpected sample config error: {err}"),
    }
}
