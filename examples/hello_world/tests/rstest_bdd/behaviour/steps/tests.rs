//! Tests for composing declarative globals and validating step behaviours.
use crate::behaviour::harness::Harness;
use crate::fixtures::hello_world_harness;
use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use rstest::rstest;

#[rstest]
#[case(
    r#"[
        {"provenance": "unknown", "value": {"foo": "bar"}}
    ]"#,
    "unknown provenance",
    "expected provenance error when composing declarative globals"
)]
#[case(
    "not valid json",
    "valid JSON",
    "expected JSON parsing error when composing declarative globals"
)]
fn compose_declarative_globals_rejects_invalid_input(
    #[case] input: &str,
    #[case] expected_message_fragment: &str,
    #[case] error_context: &str,
    hello_world_harness: Result<Harness>,
) -> Result<()> {
    let mut harness = hello_world_harness?;
    let result = super::compose_declarative_globals_from_contents(&mut harness, input);
    let Err(err) = result else {
        return Err(anyhow!("{error_context}"));
    };
    anyhow::ensure!(err.to_string().contains(expected_message_fragment));
    Ok(())
}

#[rstest]
fn environment_contains_rejects_blank_key(hello_world_harness: Result<Harness>) -> Result<()> {
    let mut harness = hello_world_harness?;
    let err = super::environment_contains(&mut harness, "  ".into(), "value".into())
        .expect_err("blank keys must be rejected");
    anyhow::ensure!(err.to_string().contains("must not be empty"));
    Ok(())
}

#[rstest]
fn run_without_args_errors_on_missing_binary(hello_world_harness: Result<Harness>) -> Result<()> {
    let mut harness = hello_world_harness?;
    harness.set_binary_override(Utf8PathBuf::from("/missing/hello_world"));
    let err = super::run_without_args(&mut harness).expect_err("missing binary should fail");
    anyhow::ensure!(err.to_string().contains("spawn"));
    Ok(())
}
