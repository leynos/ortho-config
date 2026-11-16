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
use super::super::schema::{RouteHints, SchemaField};
use super::{required_leaf, required_object};
use crate::SupplyRoute;

/// Return the routes of the only requirement `field` produces under `prefix`.
///
/// Each case needs the same three steps: hint the collector, insist that one
/// requirement is reported, and take its routes. One helper owns them so a
/// case states only the schema and the prefix it is about.
fn requirement_routes(field: SchemaField, prefix: Option<&str>) -> Vec<SupplyRoute> {
    let fields = vec![field];
    let hints = RouteHints {
        prefix: prefix.map(String::from),
        cli_flags: Vec::new(),
    };
    collect_missing(&fields, &json!({}), &hints, None)
        .expect("the requirement is absent")
        .items()
        .first()
        .expect("exactly one requirement is reported")
        .routes()
        .to_vec()
}

/// Return the environment variable advertised among `routes`, if any.
fn environment_variable(routes: &[SupplyRoute]) -> Option<&str> {
    routes.iter().find_map(|route| match route {
        SupplyRoute::Environment { variable } => Some(variable.as_str()),
        _ => None,
    })
}

#[test]
fn environment_routes_uppercase_and_nest_segments() {
    let routes = requirement_routes(
        required_object("database", vec![required_leaf("host")]),
        Some("DEMO_"),
    );
    assert_eq!(
        environment_variable(&routes),
        Some("DEMO_DATABASE__HOST"),
        "a nested leaf must advertise the uppercased, `__`-joined variable"
    );
}

/// A key the provider cannot read back is never advertised.
///
/// `environment_routes_uppercase_and_nest_segments` is the positive control:
/// the same collection passes over an expressible key and does advertise a
/// variable, so a rejection here is a property of the key and not of the
/// harness.
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
    let routes = requirement_routes(required_leaf(key), None);
    assert!(
        environment_variable(&routes).is_none(),
        "`{key}` must not advertise an environment route: {reason}"
    );
}

/// A prefix is part of the name, so an unusable prefix loses the route.
///
/// The provider trims a variable's name before matching the prefix and no name
/// may contain `=` or NUL, so a prefix carrying either, or one that only
/// matches an untrimmed name, names a variable the loader can never read. The
/// first three cases are the positive controls: the same leaf keeps a truthful
/// route whenever its prefix forms an observable name, including no prefix at
/// all and an empty one, which matches every variable without stripping
/// anything. The configuration-file route names no environment variable, so it
/// must survive every case.
#[rstest]
#[case::prefixed(Some("DEMO_"), Some("DEMO_HOST"))]
#[case::no_prefix(None, Some("HOST"))]
#[case::empty_prefix(Some(""), Some("HOST"))]
#[case::equals(Some("DEMO="), None)]
#[case::nul(Some("DEMO\0"), None)]
#[case::untrimmed(Some(" DEMO_"), None)]
fn a_prefix_that_cannot_name_a_variable_has_no_environment_route(
    #[case] prefix: Option<&str>,
    #[case] expected: Option<&str>,
) {
    let routes = requirement_routes(required_leaf("host"), prefix);
    assert_eq!(
        environment_variable(&routes),
        expected,
        "unexpected environment route for prefix {prefix:?}"
    );
    assert!(
        routes
            .iter()
            .any(|route| matches!(route, SupplyRoute::ConfigurationFile { .. })),
        "the configuration-file route must survive prefix {prefix:?}"
    );
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
    let routes = requirement_routes(
        required_object(parent, vec![required_leaf("host")]),
        Some("DEMO_"),
    );
    assert_eq!(
        environment_variable(&routes),
        expected,
        "unexpected environment route for a leaf under `{parent}`"
    );
}
