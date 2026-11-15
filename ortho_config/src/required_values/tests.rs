//! Unit tests for requirement schema composition and collection.

use serde_json::json;

use super::collect::collect_missing;
use super::schema::{
    Activation, CliFlagHint, RequiredConfigSchema, RequiredSchemaError, RequiredSchemaStack,
    RouteHints, SchemaChildren, SchemaField,
};
use super::{ensure_no_missing_required_values, program_label_from_args};
use crate::SupplyRoute;

/// A leaf requirement.
fn required_leaf(key: &str) -> SchemaField {
    SchemaField::new(key, Vec::new(), Activation::Required, SchemaChildren::Leaf)
}

/// An optional leaf requirement.
fn optional_leaf(key: &str) -> SchemaField {
    SchemaField::new(key, Vec::new(), Activation::Optional, SchemaChildren::Leaf)
}

/// A required nested object.
fn required_object(key: &str, children: Vec<SchemaField>) -> SchemaField {
    SchemaField::new(
        key,
        Vec::new(),
        Activation::Required,
        SchemaChildren::Nested(children),
    )
}

/// An optional nested object.
fn optional_object(key: &str, children: Vec<SchemaField>) -> SchemaField {
    SchemaField::new(
        key,
        Vec::new(),
        Activation::Optional,
        SchemaChildren::Nested(children),
    )
}

/// Collect missing paths from a schema and a value.
fn missing_paths(fields: &[SchemaField], value: &serde_json::Value) -> Vec<String> {
    collect_missing(fields, value, &RouteHints::default(), None)
        .map(|report| {
            report
                .items()
                .iter()
                .map(|item| item.path().render())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_absent_required_leaf_is_reported_in_declaration_order() {
    let fields = vec![required_leaf("reference"), required_leaf("region")];
    assert_eq!(
        missing_paths(&fields, &json!({})),
        vec!["reference", "region"]
    );
}

#[test]
fn present_values_including_null_and_empty_are_not_missing() {
    let fields = vec![
        required_leaf("reference"),
        required_leaf("tags"),
        required_leaf("note"),
    ];
    let value = json!({"reference": "", "tags": [], "note": null});
    assert!(
        missing_paths(&fields, &value).is_empty(),
        "every present value must count as supplied, including `null` and empty collections"
    );
}

#[test]
fn an_absent_optional_node_deactivates_its_subtree() {
    let fields = vec![optional_object(
        "database",
        vec![required_leaf("host"), required_leaf("port")],
    )];
    assert!(
        missing_paths(&fields, &json!({})).is_empty(),
        "an absent optional node must not activate its subtree"
    );
}

#[test]
fn an_absent_required_object_reports_its_required_descendants() {
    let fields = vec![required_object(
        "database",
        vec![required_leaf("host"), optional_leaf("port")],
    )];
    assert_eq!(missing_paths(&fields, &json!({})), vec!["database.host"]);
}

#[test]
fn an_absent_required_object_without_required_children_reports_itself() {
    let fields = vec![required_object(
        "tuning",
        vec![optional_leaf("threads"), optional_leaf("cache")],
    )];
    assert_eq!(missing_paths(&fields, &json!({})), vec!["tuning"]);
}

#[test]
fn a_present_object_reports_only_the_leaves_it_lacks() {
    let fields = vec![required_object(
        "database",
        vec![required_leaf("host"), required_leaf("port")],
    )];
    assert_eq!(
        missing_paths(&fields, &json!({"database": {"host": "db"}})),
        vec!["database.port"]
    );
}

#[test]
fn a_present_non_object_is_left_to_the_deserializer() {
    let fields = vec![required_object("database", vec![required_leaf("host")])];
    assert_eq!(
        missing_paths(&fields, &json!({"database": "not-an-object"})),
        Vec::<String>::new()
    );
}

#[test]
fn aliases_satisfy_a_requirement() {
    let fields = vec![SchemaField::new(
        "reference",
        vec![String::from("ref")],
        Activation::Required,
        SchemaChildren::Leaf,
    )];
    assert!(
        missing_paths(&fields, &json!({"ref": "x"})).is_empty(),
        "an alias must satisfy the requirement it stands in for"
    );
}

#[test]
fn deep_objects_are_walked_to_the_declared_depth() {
    let fields = vec![required_object(
        "a",
        vec![required_object("b", vec![required_leaf("c")])],
    )];
    assert_eq!(missing_paths(&fields, &json!({})), vec!["a.b.c"]);
    assert_eq!(
        missing_paths(&fields, &json!({"a": {"b": {}}})),
        vec!["a.b.c"]
    );
}

#[test]
fn a_non_object_root_is_not_diagnosed() {
    assert!(
        missing_paths(&[required_leaf("url")], &json!(null)).is_empty(),
        "a non-object root is the deserializer's error, not a missing value"
    );
}

#[test]
fn cli_hints_apply_only_to_top_level_leaves() {
    let fields = vec![
        required_leaf("reference"),
        required_leaf("database"),
        required_object("server", vec![required_leaf("host")]),
    ];
    let hints = RouteHints {
        prefix: Some(String::from("DEMO_")),
        cli_flags: vec![
            CliFlagHint {
                key: String::from("reference"),
                flag: String::from("--reference"),
                takes_value: true,
            },
            CliFlagHint {
                key: String::from("host"),
                flag: String::from("--host"),
                takes_value: true,
            },
        ],
    };
    let report =
        collect_missing(&fields, &json!({}), &hints, None).expect("three requirements are absent");

    let render = |route: &SupplyRoute| match route {
        SupplyRoute::CommandLine { flag, .. } => Some(flag.clone()),
        _ => None,
    };
    let flags: Vec<String> = report
        .items()
        .iter()
        .flat_map(|item| item.routes().iter())
        .filter_map(render)
        .collect();
    assert_eq!(flags, vec![String::from("--reference")]);

    let nested = report
        .items()
        .get(2)
        .expect("the nested requirement is reported third");
    assert_eq!(nested.path().render(), "server.host");
    assert!(nested.routes().iter().all(|route| matches!(
        route,
        SupplyRoute::Environment { .. } | SupplyRoute::ConfigurationFile { .. }
    )));
}

#[test]
fn environment_routes_uppercase_and_nest_segments() {
    let fields = vec![required_object("database", vec![required_leaf("host")])];
    let hints = RouteHints {
        prefix: Some(String::from("DEMO_")),
        cli_flags: Vec::new(),
    };
    let report =
        collect_missing(&fields, &json!({}), &hints, None).expect("the nested leaf is absent");
    let item = report
        .items()
        .first()
        .expect("the nested leaf is the only reported requirement");
    let variables: Vec<&str> = item
        .routes()
        .iter()
        .filter_map(|route| match route {
            SupplyRoute::Environment { variable } => Some(variable.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(variables, vec!["DEMO_DATABASE__HOST"]);
}

#[test]
fn a_dotted_segment_has_no_environment_route() {
    let fields = vec![required_leaf("a.b")];
    let report = collect_missing(&fields, &json!({}), &RouteHints::default(), None)
        .expect("the leaf is absent");
    let item = report
        .items()
        .first()
        .expect("the dotted leaf is the only reported requirement");
    assert!(
        item.routes()
            .iter()
            .all(|route| !matches!(route, SupplyRoute::Environment { .. }))
    );
}

/// A schema type whose composition refuses to terminate.
struct Recursive;

impl RequiredConfigSchema for Recursive {
    fn collect_required_schema(
        stack: &mut RequiredSchemaStack,
    ) -> Result<Vec<SchemaField>, RequiredSchemaError> {
        stack.enter::<Self>()?;
        let fields = Self::collect_required_schema(stack)?;
        stack.leave::<Self>();
        Ok(fields)
    }
}

#[test]
fn a_recursive_schema_reports_a_validation_error_instead_of_overflowing() {
    let error = ensure_no_missing_required_values::<Recursive>(&json!({}), "demo")
        .expect_err("a recursive schema cannot be composed");
    assert!(
        error
            .to_string()
            .contains("recursive required-value schema"),
        "unexpected error: {error}"
    );
}

/// One half of a cycle that only closes through another type.
struct IndirectA;

/// The other half of the same cycle.
struct IndirectB;

impl RequiredConfigSchema for IndirectA {
    fn collect_required_schema(
        stack: &mut RequiredSchemaStack,
    ) -> Result<Vec<SchemaField>, RequiredSchemaError> {
        stack.enter::<Self>()?;
        let composed = IndirectB::collect_required_schema(stack);
        stack.leave::<Self>();
        Ok(vec![required_object("b", composed?)])
    }
}

impl RequiredConfigSchema for IndirectB {
    fn collect_required_schema(
        stack: &mut RequiredSchemaStack,
    ) -> Result<Vec<SchemaField>, RequiredSchemaError> {
        stack.enter::<Self>()?;
        let composed = IndirectA::collect_required_schema(stack);
        stack.leave::<Self>();
        composed
    }
}

#[test]
fn an_indirect_cycle_is_detected_at_the_repeated_type() {
    let error = ensure_no_missing_required_values::<IndirectA>(&json!({}), "demo")
        .expect_err("a mutually recursive schema cannot be composed");
    let rendered = error.to_string();
    assert!(
        rendered.contains("recursive required-value schema"),
        "unexpected error: {rendered}"
    );
    assert_eq!(
        rendered.matches("IndirectA").count(),
        2,
        "the chain must end at the repeated type: {rendered}"
    );
    assert_eq!(
        rendered.matches("IndirectB").count(),
        1,
        "the intermediate type must appear once: {rendered}"
    );
}

#[test]
fn enter_and_leave_allow_a_type_at_two_sibling_depths() {
    let mut stack = RequiredSchemaStack::new();
    stack.enter::<u8>().expect("first entry cannot cycle");
    stack.leave::<u8>();
    stack
        .enter::<u8>()
        .expect("a sibling occurrence is not a cycle");
    assert_eq!(stack.chain().len(), 1);
}

#[test]
fn schema_children_distinguishes_leaves_from_objects() {
    assert_eq!(required_leaf("x").children, SchemaChildren::Leaf);
    assert!(matches!(
        required_object("x", Vec::new()).children,
        SchemaChildren::Nested(_)
    ));
}

#[test]
fn program_labels_come_from_the_invoked_path() {
    use std::ffi::OsString;

    assert_eq!(
        program_label_from_args(&[OsString::from("/usr/local/bin/demo")]).as_deref(),
        Some("demo")
    );
    assert_eq!(
        program_label_from_args(&[OsString::from("demo")]).as_deref(),
        Some("demo")
    );
    assert_eq!(program_label_from_args(&[OsString::from("")]), None);
    assert_eq!(program_label_from_args(&[]), None);
}
