//! Tests that validate generated declarative merge implementation tokens.

use anyhow::{Result, ensure};
use rstest::rstest;

use crate::derive::build::CollectionStrategies;
use crate::derive::generate::declarative::generate_declarative_merge_impl;

use super::helpers::{
    append_strategies, default_krate, expected_declarative_merge_impl_empty, parse_ident,
    parse_type,
};

#[rstest]
fn generate_declarative_merge_impl_emits_trait_impl() -> Result<()> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let tokens = generate_declarative_merge_impl(
        &state_ident,
        &config_ident,
        &CollectionStrategies::default(),
        &krate,
    );
    let expected = expected_declarative_merge_impl_empty()?;
    let actual = tokens.to_string();
    let expected_rendered = expected.to_string();
    ensure!(
        actual == expected_rendered,
        "declarative merge impl tokens mismatch\nactual:\n{actual}\nexpected:\n{expected_rendered}"
    );
    Ok(())
}

#[rstest]
fn generate_declarative_merge_impl_handles_append_fields() -> Result<()> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let strategies = append_strategies(vec![(parse_ident("items")?, parse_type("String")?)]);
    let tokens = generate_declarative_merge_impl(&state_ident, &config_ident, &strategies, &krate);
    let norm = tokens.to_string().replace(" :: ", "::").replace(' ', "");
    ensure!(
        norm.contains("append_items"),
        "expected append_items merge logic"
    );
    ensure!(
        norm.contains("from_value_merge"),
        "expected from_value_merge usage for append field deserialization"
    );
    ensure!(
        norm.contains("serde_json::Map::new"),
        "expected serde_json map instantiation"
    );
    ensure!(
        norm.contains("Value::Array"),
        "expected Value::Array handling"
    );
    ensure!(
        norm.contains("message.push_str(\"Source:\")"),
        "expected diagnostic source message"
    );
    Ok(())
}

fn map_only_strategies() -> Result<CollectionStrategies> {
    Ok(CollectionStrategies {
        append: Vec::new(),
        map_replace: vec![(
            parse_ident("rules")?,
            parse_type("std::collections::BTreeMap<String, u32>")?,
        )],
    })
}

fn mixed_strategies() -> Result<CollectionStrategies> {
    Ok(CollectionStrategies {
        append: vec![(parse_ident("items")?, parse_type("String")?)],
        map_replace: vec![(
            parse_ident("rules")?,
            parse_type("std::collections::BTreeMap<String, u32>")?,
        )],
    })
}

#[rstest]
#[case(
    map_only_strategies as fn() -> Result<CollectionStrategies>,
    vec!["replace_rules", "serde_json::Map::new"],
    None,
)]
#[case(
    mixed_strategies as fn() -> Result<CollectionStrategies>,
    vec!["replace_rules", "append_items", "serde_json::Map::new"],
    Some(("replace_rules", "append_items")),
)]
fn generate_declarative_merge_impl_handles_map_fields(
    #[case] strategies_fn: fn() -> Result<CollectionStrategies>,
    #[case] expected_tokens: Vec<&'static str>,
    #[case] ordering: Option<(&'static str, &'static str)>,
) -> Result<()> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let strategies = strategies_fn()?;
    let norm = generate_declarative_merge_impl(&state_ident, &config_ident, &strategies, &krate)
        .to_string()
        .replace(" :: ", "::")
        .replace(' ', "");
    for token in expected_tokens {
        ensure!(norm.contains(token), "expected merge logic for {token}",);
    }
    if let Some((first, second)) = ordering {
        let first_index = norm.find(first).expect("first merge logic should render");
        let second_index = norm.find(second).expect("second merge logic should render");
        ensure!(
            first_index < second_index,
            "expected {first} logic before {second}",
        );
    }
    Ok(())
}

#[rstest]
fn generate_declarative_merge_impl_emits_non_object_error_context() -> Result<()> {
    let krate = default_krate();
    let state_ident = parse_ident("__SampleDeclarativeMergeState")?;
    let config_ident = parse_ident("Sample")?;
    let norm = generate_declarative_merge_impl(
        &state_ident,
        &config_ident,
        &CollectionStrategies::default(),
        &krate,
    )
    .to_string()
    .replace(" :: ", "::")
    .replace(' ', "");
    ensure!(
        norm.contains("\"{provenance_label}layersupplied{value_kind}.\""),
        "guard must cite provenance and value kind: {norm}"
    );
    ensure!(
        norm.contains("message.push_str(\"Source:\")"),
        "guard must mention source paths: {norm}"
    );
    ensure!(
        norm.contains("Non-objectlayerswouldoverwriteaccumulatedstate"),
        "guard must warn about overwriting state: {norm}"
    );
    Ok(())
}
