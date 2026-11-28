//! Agent-context coverage for positional invocation details.
//!
//! Operand order is part of a command's invocation contract, so the bridge
//! carries the one-based index and the value name and lists operands ahead of
//! named flags.

use super::bridge_ir_to_agent_context;
use super::support::*;
use crate::schema::{CliMetadata, FieldMetadata, PositionalMetadata, ValueType};

/// A positional field with the given one-based index.
fn positional(name: &str, index: u16) -> FieldMetadata {
    field(FieldParts {
        name,
        value: Some(ValueType::String),
        default: None,
        required: true,
        cli: Some(CliMetadata {
            long: None,
            short: None,
            value_name: Some(name.to_uppercase()),
            multiple: false,
            takes_value: true,
            value_optional: false,
            possible_values: Vec::new(),
            hide_in_help: false,
            positional: Some(PositionalMetadata::new(index)),
        }),
    })
}

/// A named flag field.
fn named(name: &str) -> FieldMetadata {
    field(FieldParts {
        name,
        value: Some(ValueType::String),
        default: None,
        required: false,
        cli: Some(CliMetadata {
            long: Some(name.to_owned()),
            short: None,
            value_name: Some(name.to_uppercase()),
            multiple: false,
            takes_value: true,
            value_optional: false,
            possible_values: Vec::new(),
            hide_in_help: false,
            positional: None,
        }),
    })
}

#[test]
fn positional_inputs_are_ordered_by_index_and_carry_the_index() {
    let metadata = doc(DocSpec {
        app_name: "demo",
        bin_name: Some("demo-bin"),
        about_id: "root.about",
        fields: vec![
            named("mode"),
            positional("second", 2),
            positional("first", 1),
        ],
        subcommands: Vec::new(),
        behaviour: None,
    });

    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None).expect("bridge succeeds");
    let command = context
        .commands
        .first()
        .expect("the root command should be generated");
    let inputs = &command.inputs;
    let order: Vec<_> = inputs.iter().map(|input| input.name.as_str()).collect();
    assert_eq!(
        order,
        ["first", "second", "mode"],
        "operands should lead in index order, followed by named flags"
    );
    let first = inputs.first().expect("the first operand should be present");
    let flag = inputs
        .get(2)
        .expect("the named flag should follow the operands");
    assert_eq!(
        first.positional,
        Some(1),
        "the operand should carry its one-based index"
    );
    assert_eq!(
        first.value_name.as_deref(),
        Some("FIRST"),
        "the operand should carry its value name"
    );
    assert_eq!(
        flag.positional, None,
        "a named flag should not carry a positional index"
    );
}
