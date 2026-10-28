//! Field-declaration tests for `#[ortho_config(renderer(...))]`.
//!
//! Output fields are the only renderer leaf that accepts a repeated key, so
//! their validation is separated from the policy-leaf tests: an identifier
//! must be non-empty and unique within one record, and its localization token
//! must be spelled exactly.

use super::renderer_attrs::{assert_rejected, parse_renderer};
use anyhow::{Context, Result, ensure};

/// A declared output field keeps its identifier and localization token.
#[test]
fn parses_declared_field() -> Result<()> {
    let renderer = parse_renderer(
        r#"renderer(machine(field("error_code", localization = "non_localized")))"#,
    )?;
    let machine = renderer.machine.context("expected a machine record")?;
    let [field] = machine.fields.as_slice() else {
        return Err(anyhow::anyhow!("fields: {:?}", machine.fields.len()));
    };
    ensure!(field.id == "error_code", "id: {:?}", field.id);
    ensure!(
        field.localization == "non_localized",
        "localization: {:?}",
        field.localization
    );
    Ok(())
}

/// Fields keep source order, since the emitted IR preserves declaration order.
#[test]
fn preserves_field_declaration_order() -> Result<()> {
    let renderer = parse_renderer(
        r#"renderer(human(
            field("zeta_message", localization = "localized"),
            field("alpha_code", localization = "non_localized")
        ))"#,
    )?;
    let human = renderer.human.context("expected a human record")?;
    let ids: Vec<&str> = human.fields.iter().map(|field| field.id.as_str()).collect();
    ensure!(ids == ["zeta_message", "alpha_code"], "ids: {ids:?}");
    Ok(())
}

/// Unicode identifiers are accepted verbatim: the parser never normalizes them.
#[test]
fn accepts_unicode_field_identifier() -> Result<()> {
    let renderer =
        parse_renderer("renderer(machine(field(\"échec\", localization = \"localized\")))")?;
    let machine = renderer.machine.context("expected a machine record")?;
    let [field] = machine.fields.as_slice() else {
        return Err(anyhow::anyhow!("expected one field"));
    };
    ensure!(field.id == "échec", "id: {:?}", field.id);
    Ok(())
}

/// An empty field identifier is rejected at the identifier's own span.
#[test]
fn rejects_empty_field_identifier() {
    assert_rejected(
        r#"renderer(human(field("", localization = "localized")))"#,
        "field id must not be empty",
    );
}

/// A field declared without a localization token is rejected.
#[test]
fn rejects_field_without_localization() {
    assert_rejected(
        r#"renderer(human(field("error_code")))"#,
        "field requires localization",
    );
}

/// A field with an unknown localization token is rejected.
#[test]
fn rejects_field_with_unknown_localization() {
    assert_rejected(
        r#"renderer(human(field("error_code", localization = "maybe")))"#,
        "unknown localization 'maybe'",
    );
}

/// A field with an unrecognized policy key is rejected.
#[test]
fn rejects_field_with_unknown_policy() {
    assert_rejected(
        r#"renderer(human(field("error_code", width = "localized")))"#,
        "unknown field policy `width`",
    );
}

/// Repeating a policy within one field is rejected.
#[test]
fn rejects_repeated_field_policy() {
    assert_rejected(
        r#"renderer(human(field(
            "error_code",
            localization = "localized",
            localization = "non_localized"
        )))"#,
        "duplicate field policy `localization`",
    );
}

/// Two fields sharing an identifier are rejected: identifiers are stable keys.
#[test]
fn rejects_duplicate_field_identifier() {
    assert_rejected(
        r#"renderer(machine(
            field("error_code", localization = "localized"),
            field("error_code", localization = "non_localized")
        ))"#,
        "duplicate output field 'error_code'",
    );
}

/// The same identifier in different records is allowed: records are independent.
#[test]
fn allows_same_field_identifier_in_both_records() -> Result<()> {
    let renderer = parse_renderer(
        r#"renderer(
            human(field("error_code", localization = "localized")),
            machine(field("error_code", localization = "non_localized"))
        )"#,
    )?;
    let human = renderer.human.context("expected a human record")?;
    let machine = renderer.machine.context("expected a machine record")?;
    let [human_field] = human.fields.as_slice() else {
        return Err(anyhow::anyhow!("expected one human field"));
    };
    let [machine_field] = machine.fields.as_slice() else {
        return Err(anyhow::anyhow!("expected one machine field"));
    };
    ensure!(
        human_field.localization == "localized",
        "human localization"
    );
    ensure!(
        machine_field.localization == "non_localized",
        "machine localization"
    );
    Ok(())
}
