//! Tests that validate generated declarative state struct tokens.

use anyhow::{Result, ensure};
use rstest::rstest;

use crate::derive::build::CollectionStrategies;
use crate::derive::generate::declarative::generate_declarative_state_struct;

use super::helpers::{append_strategies, default_krate, parse_ident, parse_type};

#[rstest]
fn generate_declarative_state_struct_emits_storage() -> Result<()> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("SampleConfig")?;
    let tokens = generate_declarative_state_struct(
        &state_ident,
        &config_ident,
        &CollectionStrategies::default(),
        &krate,
    );
    let rendered = tokens.to_string();
    ensure!(
        rendered.contains("__SampleDeclarativeMergeState"),
        "struct name present"
    );
    ensure!(
        rendered.contains("Declarative merge state generated") && rendered.contains("SampleConfig"),
        "doc comment should mention role and config: {rendered}"
    );
    ensure!(
        rendered.contains("serde_json :: Value") || rendered.contains("serde_json::Value"),
        "value field should be rendered: {rendered}"
    );
    Ok(())
}

#[rstest]
#[case(
    append_strategies(vec![(parse_ident("items")?, parse_type("String")?)]),
    vec!["append_items"],
)]
#[case(
    CollectionStrategies {
        append: Vec::new(),
        map_replace: vec![
            (
                parse_ident("rules")?,
                parse_type("std::collections::BTreeMap<String, u32>")?,
            ),
        ],
    },
    vec!["replace_rules"],
)]
#[case(
    CollectionStrategies {
        append: vec![(parse_ident("items")?, parse_type("String")?)],
        map_replace: vec![
            (
                parse_ident("rules")?,
                parse_type("std::collections::BTreeMap<String, u32>")?,
            ),
        ],
    },
    vec!["append_items", "replace_rules"],
)]
fn generate_declarative_state_struct_includes_collection_fields(
    #[case] strategies: CollectionStrategies,
    #[case] expected_fields: Vec<&'static str>,
) -> Result<()> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("SampleConfig")?;
    let tokens =
        generate_declarative_state_struct(&state_ident, &config_ident, &strategies, &krate);
    let norm = tokens.to_string().replace(' ', "");
    for field in expected_fields {
        ensure!(
            norm.contains(field),
            "expected state struct to include {field}",
        );
    }
    Ok(())
}
