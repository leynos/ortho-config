//! Tests for composing declarative globals in the hello world example.
use crate::behaviour::harness::Harness;
use anyhow::{anyhow, Result};
use rstest::{fixture, rstest};

#[fixture]
fn harness() -> Result<Harness> {
    Harness::for_tests()
}

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
    harness: Result<Harness>,
) -> Result<()> {
    let mut harness = harness?;
    let result = super::compose_declarative_globals_from_contents(&mut harness, input);
    let Err(err) = result else {
        return Err(anyhow!("{error_context}"));
    };
    anyhow::ensure!(err.to_string().contains(expected_message_fragment));
    Ok(())
}
