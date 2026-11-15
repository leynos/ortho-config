//! Which supply routes a reported requirement may advertise.
//!
//! Every case here drives the collector and inspects the route list it
//! produces. An environment route is truthful only when the configured
//! provider reads the spelled variable back as the reported key, so the cases
//! pair rejected spellings with their positive controls: without a control, a
//! route missing from every case would look like a rejection.

use rstest::rstest;
use serde_json::json;

use super::super::collect::collect_missing;
use super::super::schema::RouteHints;
use super::{required_leaf, required_object};
use crate::SupplyRoute;

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

/// Assert the sole reported requirement for `key` has no environment route.
///
/// `environment_routes_uppercase_and_nest_segments` is the positive control:
/// the same collection passes over an expressible key and does advertise a
/// variable, so a rejection here is a property of the key and not of the
/// harness.
fn assert_no_environment_route(key: &str, reason: &str) {
    let fields = vec![required_leaf(key)];
    let report = collect_missing(&fields, &json!({}), &RouteHints::default(), None)
        .expect("the leaf is absent");
    let item = report
        .items()
        .first()
        .expect("the leaf is the only reported requirement");
    assert!(
        item.routes()
            .iter()
            .all(|route| !matches!(route, SupplyRoute::Environment { .. })),
        "`{key}` must not advertise an environment route: {reason}"
    );
}

#[rstest]
#[case::dotted("a.b", "a literal dot nests instead of naming one key")]
#[case::split("a__b", "an embedded `__` splits into two segments")]
#[case::trimmed(" a", "surrounding whitespace is trimmed away")]
#[case::uppercase("aB", "the final transform lowercases the key")]
#[case::equals("a=b", "`=` separates a variable's name from its value")]
#[case::nul("a\0b", "a NUL terminates the name the platform call receives")]
fn keys_the_provider_cannot_express_have_no_environment_route(
    #[case] key: &str,
    #[case] reason: &str,
) {
    assert_no_environment_route(key, reason);
}

/// Return the environment variable advertised for a nested leaf under `parent`.
fn nested_environment_variable(parent: &str) -> Option<String> {
    let fields = vec![required_object(parent, vec![required_leaf("host")])];
    let hints = RouteHints {
        prefix: Some(String::from("DEMO_")),
        cli_flags: Vec::new(),
    };
    let report =
        collect_missing(&fields, &json!({}), &hints, None).expect("the nested leaf is absent");
    report
        .items()
        .first()
        .expect("the nested leaf is the only reported requirement")
        .routes()
        .iter()
        .find_map(|route| match route {
            SupplyRoute::Environment { variable } => Some(variable.clone()),
            _ => None,
        })
}

/// A segment edge that abuts the separator changes the path the provider sees.
///
/// The provider splits the joined name, not the segments, so `database_` plus
/// `__` becomes `database___HOST`, which it reads as `database._host`. The
/// expressible and leading-underscore cases are the positive controls: only
/// an edge that merges with the separator loses its route, and a leading
/// underscore, which the prefix strip exposes rather than absorbs, keeps one.
#[rstest]
#[case::expressible("database", Some("DEMO_DATABASE__HOST"))]
#[case::leading_underscore("_database", Some("DEMO__DATABASE__HOST"))]
#[case::trailing_underscore("database_", None)]
fn a_segment_edge_that_merges_with_the_separator_has_no_environment_route(
    #[case] parent: &str,
    #[case] expected: Option<&str>,
) {
    assert_eq!(
        nested_environment_variable(parent).as_deref(),
        expected,
        "unexpected environment route for a leaf under `{parent}`"
    );
}
