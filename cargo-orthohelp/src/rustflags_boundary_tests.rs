//! Command-boundary tests for applying sanitized Rust flag variables.
//!
//! The property tests exercise the pure token transformations. These cases
//! verify that `apply_sanitized_rustflags` transfers their results to a child
//! command, including removing variables that contain only coverage flags.

use std::ffi::OsStr;
use std::process::Command;

use rstest::rstest;
use test_helpers::env::EnvScope;

use super::apply_sanitized_rustflags;

#[derive(Clone, Copy, Debug)]
enum ExpectedEnvironmentValue {
    Removed,
    Set(&'static str),
}

#[rstest]
#[case(
    "-C instrument-coverage --cfg plain_gate",
    ExpectedEnvironmentValue::Set("--cfg plain_gate"),
    "--cfg\u{1f}encoded_gate",
    ExpectedEnvironmentValue::Set("--cfg\u{1f}encoded_gate")
)]
#[case(
    "-Cinstrument-coverage",
    ExpectedEnvironmentValue::Removed,
    "--cfg\u{1f}encoded_gate",
    ExpectedEnvironmentValue::Set("--cfg\u{1f}encoded_gate")
)]
#[case(
    "  --cfg   plain_gate  ",
    ExpectedEnvironmentValue::Set("  --cfg   plain_gate  "),
    "--cfg\u{1f}encoded_gate",
    ExpectedEnvironmentValue::Set("--cfg\u{1f}encoded_gate")
)]
#[case(
    "--cfg plain_gate",
    ExpectedEnvironmentValue::Set("--cfg plain_gate"),
    "-Cinstrument-coverage\u{1f}--cfg\u{1f}encoded_gate",
    ExpectedEnvironmentValue::Set("--cfg\u{1f}encoded_gate")
)]
#[case(
    "--cfg plain_gate",
    ExpectedEnvironmentValue::Set("--cfg plain_gate"),
    "-C\u{1f}instrument-coverage=once",
    ExpectedEnvironmentValue::Removed
)]
#[case(
    "--cfg plain_gate",
    ExpectedEnvironmentValue::Set("--cfg plain_gate"),
    "--cfg\u{1f}encoded_gate",
    ExpectedEnvironmentValue::Set("--cfg\u{1f}encoded_gate")
)]
fn sanitized_values_are_applied_to_the_child_command(
    #[case] plain_input: &'static str,
    #[case] expected_plain: ExpectedEnvironmentValue,
    #[case] encoded_input: &'static str,
    #[case] expected_encoded: ExpectedEnvironmentValue,
) {
    let _scope = EnvScope::new_with(|lock| {
        vec![
            lock.set_var("RUSTFLAGS", OsStr::new(plain_input)),
            lock.set_var("CARGO_ENCODED_RUSTFLAGS", OsStr::new(encoded_input)),
        ]
    });

    let mut command = Command::new("bridge");
    apply_sanitized_rustflags(&mut command);

    let command_env = command.get_envs().collect::<Vec<_>>();
    assert_environment_value(&command_env, OsStr::new("RUSTFLAGS"), expected_plain);
    assert_environment_value(
        &command_env,
        OsStr::new("CARGO_ENCODED_RUSTFLAGS"),
        expected_encoded,
    );
}

fn assert_environment_value(
    command_env: &[(&OsStr, Option<&OsStr>)],
    name: &OsStr,
    expected: ExpectedEnvironmentValue,
) {
    let has_expected_value = command_env
        .iter()
        .find(|(key, _)| *key == name)
        .is_some_and(|(_, value)| match expected {
            ExpectedEnvironmentValue::Removed => value.is_none(),
            ExpectedEnvironmentValue::Set(expected_value) => value
                .as_ref()
                .is_some_and(|actual| *actual == OsStr::new(expected_value)),
        });
    assert!(
        has_expected_value,
        "unexpected value for command environment variable {name:?}"
    );
}
