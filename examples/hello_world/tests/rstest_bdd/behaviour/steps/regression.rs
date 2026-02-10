//! Tests for composing declarative globals and validating step behaviours.
use crate::behaviour::harness::Harness;
use crate::fixtures::hello_world_harness;
use anyhow::Result;
use camino::Utf8PathBuf;
use rstest::rstest;

#[rstest]
#[case(
    r#"[
        {"provenance": "unknown", "value": {"foo": "bar"}}
    ]"#,
    "unknown variant",
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
    hello_world_harness: Harness,
) -> Result<()> {
    let mut harness = hello_world_harness;
    let err =
        super::global::compose_declarative_globals_from_contents_for_tests(&mut harness, input)
            .expect_err(error_context);
    let report = format!("{err:#}");
    anyhow::ensure!(
        report.contains(expected_message_fragment),
        "expected error containing '{expected_message_fragment}', got: {report}"
    );
    Ok(())
}

#[rstest]
fn environment_contains_rejects_blank_key(hello_world_harness: Harness) -> Result<()> {
    let mut harness = hello_world_harness;
    let err = super::global::environment_contains(&mut harness, "  ".into(), "value".into())
        .expect_err("blank keys must be rejected");
    anyhow::ensure!(
        err.to_string().contains("must not be empty"),
        "expected error containing 'must not be empty', got: {err}"
    );
    Ok(())
}

#[rstest]
fn run_without_args_errors_on_missing_binary(hello_world_harness: Harness) -> Result<()> {
    let mut harness = hello_world_harness;
    harness.set_binary_override(Utf8PathBuf::from("/missing/hello_world"));
    let err =
        super::global::run_without_args(&mut harness).expect_err("missing binary should fail");
    anyhow::ensure!(
        err.to_string().contains("spawn"),
        "expected error containing 'spawn', got: {err}"
    );
    Ok(())
}
