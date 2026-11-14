//! Property tests connecting the runtime collector to the abstract model.
//!
//! Each generated case declares a schema and a merged value from one
//! requirement model, computes the expected missing set independently, and
//! compares it with the collector's ordered observation. The model is the
//! subsequence of required-and-absent identities in declaration order.

use proptest::prelude::*;
use serde_json::{Map, Value, json};

use super::collect::collect_missing;
use super::schema::{Activation, CliFlagHint, RouteHints, SchemaField};
use crate::error::{MissingRequiredValue, MissingRequiredValues, SupplyRoute};

/// The environment prefix every generated route hint uses.
const PREFIX: &str = "APP_";

/// One top-level requirement's activation and observed presence.
type Case = (bool, bool);

/// Convert a generated flag into its declared activation.
fn activation(is_required: bool) -> Activation {
    if is_required {
        Activation::Required
    } else {
        Activation::Optional
    }
}

/// The key generated for the requirement at `index`.
fn key(index: usize) -> String {
    format!("f{index}")
}

/// Build the schema, merged value, and route hints for one flat case.
fn build_flat(cases: &[Case]) -> (Vec<SchemaField>, Value, RouteHints) {
    let fields: Vec<SchemaField> = cases
        .iter()
        .enumerate()
        .map(|(index, (is_required, _))| {
            SchemaField::leaf(key(index), Vec::new(), activation(*is_required))
        })
        .collect();
    let mut object = Map::new();
    for (index, (_, is_present)) in cases.iter().enumerate() {
        if *is_present {
            object.insert(key(index), json!("supplied"));
        }
    }
    let hints = RouteHints {
        prefix: Some(String::from(PREFIX)),
        cli_flags: cases
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let flag_key = key(index);
                CliFlagHint {
                    flag: format!("--{flag_key}"),
                    key: flag_key,
                    takes_value: true,
                }
            })
            .collect(),
    };
    (fields, Value::Object(object), hints)
}

/// Compute the expected missing identities without consulting the collector.
fn expected_flat(cases: &[Case]) -> Vec<String> {
    cases
        .iter()
        .enumerate()
        .filter(|(_, (is_required, is_present))| *is_required && !*is_present)
        .map(|(index, _)| key(index))
        .collect()
}

/// Run the collector and render its ordered paths.
fn observed_flat(cases: &[Case]) -> Option<Vec<String>> {
    let (fields, value, hints) = build_flat(cases);
    collect_missing(&fields, &value, &hints, Some(String::from("prop")))
        .map(|report| rendered_paths(&report))
}

/// Render every reported path in report order.
fn rendered_paths(report: &MissingRequiredValues) -> Vec<String> {
    report
        .items()
        .iter()
        .map(|item| item.path().render())
        .collect()
}

/// Assert each reported leaf advertises truthful routes in source order.
fn assert_leaf_routes(report: &MissingRequiredValues) {
    for item in report.items() {
        let Some(leaf) = leaf_segment(item) else {
            continue;
        };
        let routes = item.routes();
        assert_cli_route(routes, leaf);
        assert_environment_route(routes, leaf);
        assert_file_route(routes);
    }
}

/// Return the sole segment of a flat-model path, asserting it is a leaf.
fn leaf_segment(item: &MissingRequiredValue) -> Option<&str> {
    let segments = item.path().segments();
    assert_eq!(segments.len(), 1, "the flat model only reports leaves");
    segments.first().map(String::as_str)
}

/// Assert the command-line route comes first and matches the leaf key.
fn assert_cli_route(routes: &[SupplyRoute], leaf: &str) {
    let expected = format!("--{leaf}");
    assert!(
        matches!(
            routes.first(),
            Some(SupplyRoute::CommandLine { flag, takes_value: true }) if flag == &expected
        ),
        "expected a CLI route first: {routes:?}"
    );
}

/// Assert the environment route comes second and matches the leaf key.
fn assert_environment_route(routes: &[SupplyRoute], leaf: &str) {
    let expected = format!("{PREFIX}{}", leaf.to_ascii_uppercase());
    assert!(
        matches!(
            routes.get(1),
            Some(SupplyRoute::Environment { variable }) if variable == &expected
        ),
        "expected an environment route second: {routes:?}"
    );
}

/// Assert the configuration-file route comes last and expects a scalar.
fn assert_file_route(routes: &[SupplyRoute]) {
    assert!(
        matches!(
            routes.get(2),
            Some(SupplyRoute::ConfigurationFile {
                expects_object: false,
                ..
            })
        ),
        "expected a file route last: {routes:?}"
    );
}

proptest! {
    /// Every activated absent leaf appears once, in declaration order.
    #[test]
    fn collection_is_sound_complete_and_ordered(cases in prop::collection::vec(
        (any::<bool>(), any::<bool>()),
        0..=64,
    )) {
        let expected = expected_flat(&cases);
        let observed = observed_flat(&cases);
        prop_assert_eq!(observed.unwrap_or_default(), expected);
        let (fields, value, hints) = build_flat(&cases);
        let collected = collect_missing(&fields, &value, &hints, Some(String::from("prop")));
        if let Some(report) = collected {
            assert_leaf_routes(&report);
        }
    }

    /// Supplying one absent requirement removes exactly that identity.
    #[test]
    fn repair_removes_only_the_supplied_identity(cases in prop::collection::vec(
        (any::<bool>(), any::<bool>()),
        0..=64,
    )) {
        let expected = expected_flat(&cases);
        for (index, (is_required, is_present)) in cases.iter().enumerate() {
            if !*is_required || *is_present {
                continue;
            }
            let mut repaired = cases.clone();
            if let Some(slot) = repaired.get_mut(index) {
                slot.1 = true;
            }
            let supplied = key(index);
            let expected_after: Vec<String> = expected
                .iter()
                .filter(|remaining| **remaining != supplied)
                .cloned()
                .collect();
            let observed = observed_flat(&repaired);
            prop_assert_eq!(observed.unwrap_or_default(), expected_after);
        }
    }

    /// A present parent inspects children; an absent one follows activation.
    #[test]
    fn nested_activation_matches_the_model(
        (parent_required, parent_present, children) in (
            any::<bool>(),
            any::<bool>(),
            prop::collection::vec((any::<bool>(), any::<bool>()), 0..=8),
        )
    ) {
        let expected = expected_nested(parent_required, parent_present, &children);
        let observed = observed_nested(parent_required, parent_present, &children);
        prop_assert_eq!(observed.unwrap_or_default(), expected);
    }
}

/// Compute the expected nested identities without consulting the collector.
fn expected_nested(parent_required: bool, parent_present: bool, children: &[Case]) -> Vec<String> {
    if parent_present {
        return children
            .iter()
            .enumerate()
            .filter(|(_, (required, present))| *required && !*present)
            .map(|(index, _)| format!("parent.c{index}"))
            .collect();
    }
    if !parent_required {
        return Vec::new();
    }
    let required_children: Vec<String> = children
        .iter()
        .enumerate()
        .filter(|(_, (required, _))| *required)
        .map(|(index, _)| format!("parent.c{index}"))
        .collect();
    if required_children.is_empty() {
        vec![String::from("parent")]
    } else {
        required_children
    }
}

/// Build the nested schema and merged value, then run the collector.
fn observed_nested(
    parent_required: bool,
    parent_present: bool,
    children: &[Case],
) -> Option<Vec<String>> {
    let child_fields: Vec<SchemaField> = children
        .iter()
        .enumerate()
        .map(|(index, (required, _))| {
            SchemaField::leaf(format!("c{index}"), Vec::new(), activation(*required))
        })
        .collect();
    let parent = SchemaField::nested(
        "parent",
        Vec::new(),
        activation(parent_required),
        child_fields,
    );
    let mut inner = Map::new();
    for (index, (_, present)) in children.iter().enumerate() {
        if *present {
            inner.insert(format!("c{index}"), json!("supplied"));
        }
    }
    let mut root = Map::new();
    if parent_present {
        root.insert(String::from("parent"), Value::Object(inner));
    }
    let hints = RouteHints::default();
    collect_missing(
        std::slice::from_ref(&parent),
        &Value::Object(root),
        &hints,
        Some(String::from("prop")),
    )
    .map(|report| rendered_paths(&report))
}

#[test]
fn an_empty_schema_reports_nothing() {
    assert_eq!(observed_flat(&[]), None);
}

#[test]
fn an_all_present_schema_reports_nothing() {
    let cases = vec![(true, true), (false, true)];
    assert_eq!(observed_flat(&cases), None);
}

#[test]
fn an_all_missing_schema_reports_declaration_order() {
    let cases = vec![(true, false), (true, false)];
    assert_eq!(
        observed_flat(&cases),
        Some(vec![String::from("f0"), String::from("f1")])
    );
}

#[test]
fn seeded_omission_inversion_and_reorder_fail_the_comparison() {
    let cases = vec![(true, false), (true, true), (true, false), (false, false)];
    let actual = observed_flat(&cases).unwrap_or_default();
    assert_eq!(
        actual,
        vec![String::from("f0"), String::from("f2")],
        "the fixture must expose the faulted assertions below"
    );
    assert_ne!(actual, vec![String::from("f0")], "omission must differ");
    assert_ne!(
        actual,
        vec![String::from("f1"), String::from("f2")],
        "inversion must differ"
    );
    assert_ne!(
        actual,
        vec![String::from("f2"), String::from("f0")],
        "reordering must differ"
    );
}
