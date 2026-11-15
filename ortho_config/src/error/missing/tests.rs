//! Unit tests for the missing-required-values payload.

use super::{ConfigPath, MissingRequiredValue, MissingRequiredValues, SupplyRoute};

/// Build a leaf report entry with the given routes.
fn leaf(path: &[&str], routes: Vec<SupplyRoute>) -> MissingRequiredValue {
    MissingRequiredValue::new(ConfigPath::new(path.iter().copied()), routes, false)
}

/// Build an object report entry with the given routes.
fn object(path: &[&str], routes: Vec<SupplyRoute>) -> MissingRequiredValue {
    MissingRequiredValue::new(ConfigPath::new(path.iter().copied()), routes, true)
}

/// Build a configuration-file route for one canonical path.
#[cfg(feature = "toml")]
fn file(path: &[&str], expects_object: bool) -> SupplyRoute {
    SupplyRoute::ConfigurationFile {
        path: ConfigPath::new(path.iter().copied()),
        expects_object,
    }
}

/// Extract the Configuration File section body from a rendered report.
#[cfg(feature = "toml")]
fn configuration_file_body(rendered: &str) -> String {
    rendered
        .lines()
        .skip_while(|line| !line.ends_with("Configuration File:"))
        .skip(1)
        .take_while(|line| line.starts_with("     "))
        .map(|line| line.strip_prefix("     ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Report an object route that arrives before the leaves sharing its table.
fn report_object_before_its_leaves() -> MissingRequiredValues {
    MissingRequiredValues::try_new(
        None,
        vec![
            object(&["database"], vec![file(&["database"], true)]),
            leaf(
                &["database", "host"],
                vec![file(&["database", "host"], false)],
            ),
            leaf(
                &["database", "port"],
                vec![file(&["database", "port"], false)],
            ),
            leaf(&["reference"], vec![file(&["reference"], false)]),
        ],
    )
    .expect("four entries form a non-empty report")
}

/// Report keys whose spellings a TOML bare key cannot express.
fn report_unconventional_keys() -> MissingRequiredValues {
    let keys = [r"api@key", r#"he said "hi""#, r"back\slash", "tab\there"];
    let items: Vec<MissingRequiredValue> = keys
        .iter()
        .map(|key| leaf(&[*key], vec![file(&[*key], false)]))
        .collect();
    MissingRequiredValues::try_new(None, items).expect("four entries form a non-empty report")
}

/// Report the routes a demo program really accepts for two absent values.
fn demo_report() -> MissingRequiredValues {
    MissingRequiredValues::try_new(
        Some(String::from("demo")),
        vec![
            leaf(
                &["reference"],
                vec![
                    SupplyRoute::CommandLine {
                        flag: String::from("--reference"),
                        takes_value: true,
                    },
                    SupplyRoute::Environment {
                        variable: String::from("DEMO_REFERENCE"),
                    },
                    SupplyRoute::ConfigurationFile {
                        path: ConfigPath::new(["reference"]),
                        expects_object: false,
                    },
                ],
            ),
            leaf(
                &["database", "host"],
                vec![
                    SupplyRoute::Environment {
                        variable: String::from("DEMO_DATABASE__HOST"),
                    },
                    SupplyRoute::ConfigurationFile {
                        path: ConfigPath::new(["database", "host"]),
                        expects_object: false,
                    },
                ],
            ),
        ],
    )
    .expect("two entries form a non-empty report")
}

#[test]
fn display_groups_routes_by_supply_method() {
    let expected = concat!(
        "Missing required configuration for `demo`\n",
        "\n",
        "The following value(s) are missing:\n",
        "  - `reference`\n",
        "  - `database.host`\n",
        "\n",
        "Please provide them using one of the following methods, in order of precedence:\n",
        "  1. Command-Line Argument:\n",
        "     --reference <value>\n",
        "  2. Environment Variable:\n",
        "     DEMO_REFERENCE=<value>\n",
        "     DEMO_DATABASE__HOST=<value>\n",
        "  3. Configuration File:\n",
        "     reference = \"<value>\"\n",
        "     [database]\n",
        "     host = \"<value>\"\n",
    );
    assert_eq!(demo_report().to_string(), expected);
}

#[cfg(feature = "toml")]
#[test]
fn the_rendered_file_section_is_one_valid_toml_document() {
    // Report order puts a nested route before the root key and repeats a table,
    // which a naive per-route rendering would write as a root key inside a
    // table and a duplicate header. Grouping must keep the section parseable.
    let report = MissingRequiredValues::try_new(
        None,
        vec![
            leaf(
                &["database", "port"],
                vec![file(&["database", "port"], false)],
            ),
            leaf(
                &["server", "api.key"],
                vec![file(&["server", "api.key"], false)],
            ),
            leaf(&["reference"], vec![file(&["reference"], false)]),
            leaf(
                &["database", "host"],
                vec![file(&["database", "host"], false)],
            ),
        ],
    )
    .expect("four entries form a non-empty report");

    let body = configuration_file_body(&report.to_string());
    let parsed: toml::Table =
        toml::from_str(&body).expect("the Configuration File section must be valid TOML");

    assert!(
        parsed.contains_key("reference"),
        "a root key must stay at the document root: {body}"
    );
    let database = parsed
        .get("database")
        .and_then(toml::Value::as_table)
        .expect("the database table must exist");
    assert!(
        database.contains_key("host") && database.contains_key("port"),
        "both database keys must share one table: {body}"
    );
    let server = parsed
        .get("server")
        .and_then(toml::Value::as_table)
        .expect("the server table must exist");
    assert!(
        server.contains_key("api.key"),
        "a quoted table key must survive parsing: {body}"
    );
    assert_eq!(
        body.matches("[database]").count(),
        1,
        "a table header must appear once: {body}"
    );
}

/// An object route and the leaves inside it must cooperate.
///
/// The object route alone says "declare this table"; the leaf routes then add
/// keys to it. Rendering them independently would emit the header twice. The
/// assertion is independent of the snapshot: the expected body is written out
/// and the parsed document is inspected for the same structure.
#[cfg(feature = "toml")]
#[test]
fn an_object_route_and_its_nested_leaf_share_one_table() {
    let report = report_object_before_its_leaves();
    let body = configuration_file_body(&report.to_string());
    assert_eq!(
        body,
        concat!(
            "reference = \"<value>\"\n",
            "[database]\n",
            "host = \"<value>\"\n",
            "port = \"<value>\"",
        ),
        "root assignments must precede the table and the table must open once"
    );

    let parsed: toml::Table =
        toml::from_str(&body).expect("the Configuration File section must be valid TOML");
    assert!(
        parsed.contains_key("reference"),
        "a root key must stay at the document root: {body}"
    );
    let database = parsed
        .get("database")
        .and_then(toml::Value::as_table)
        .expect("the database table must exist");
    assert!(
        database.contains_key("host") && database.contains_key("port"),
        "the nested leaves must land inside the object's table: {body}"
    );
}

/// Every key spelling must survive a round trip through a TOML parser.
#[cfg(feature = "toml")]
#[test]
fn keys_a_bare_toml_key_cannot_express_are_quoted_and_parse() {
    let report = report_unconventional_keys();
    let body = configuration_file_body(&report.to_string());
    let parsed: toml::Table =
        toml::from_str(&body).expect("the Configuration File section must be valid TOML");
    for key in [r"api@key", r#"he said "hi""#, r"back\slash", "tab\there"] {
        assert!(
            parsed.contains_key(key),
            "`{key}` must parse back under its exact spelling: {body}"
        );
    }
}

#[test]
fn display_reports_an_absent_object_without_a_cli_route() {
    let report = MissingRequiredValues::try_new(
        None,
        vec![object(
            &["database"],
            vec![
                SupplyRoute::Environment {
                    variable: String::from("DEMO_DATABASE"),
                },
                SupplyRoute::ConfigurationFile {
                    path: ConfigPath::new(["database"]),
                    expects_object: true,
                },
            ],
        )],
    )
    .expect("one entry forms a non-empty report");

    let expected = concat!(
        "Missing required configuration\n",
        "\n",
        "The following value(s) are missing:\n",
        "  - `database`\n",
        "\n",
        "Please provide them using one of the following methods, in order of precedence:\n",
        "  1. Environment Variable:\n",
        "     DEMO_DATABASE=<value>\n",
        "  2. Configuration File:\n",
        "     [database]\n",
    );
    assert_eq!(report.to_string(), expected);
}

#[test]
fn an_empty_report_is_not_constructible() {
    assert_eq!(MissingRequiredValues::try_new(None, Vec::new()), None);
}

#[test]
fn a_boolean_flag_renders_without_a_value_placeholder() {
    let report = MissingRequiredValues::try_new(
        Some(String::from("demo")),
        vec![leaf(
            &["verbose"],
            vec![SupplyRoute::CommandLine {
                flag: String::from("--verbose"),
                takes_value: false,
            }],
        )],
    )
    .expect("one entry forms a non-empty report");
    assert!(report.to_string().contains("     --verbose\n"));
    assert!(!report.to_string().contains("--verbose <value>"));
}

#[test]
fn dotted_segments_are_quoted_to_keep_paths_unambiguous() {
    let path = ConfigPath::new(["a.b", "c"]);
    assert_eq!(path.render(), "\"a.b\".c");
    assert_eq!(ConfigPath::new(["a", "b"]).render(), "a.b");
    assert_eq!(ConfigPath::new(["with space"]).render(), "\"with space\"");
    assert_eq!(ConfigPath::new(Vec::<String>::new()).render(), "");
}

#[test]
fn keys_outside_the_bare_alphabet_are_quoted_and_escaped() {
    assert_eq!(
        ConfigPath::new(["Host=Name"]).render(),
        "\"Host=Name\"",
        "`=` is outside TOML's bare-key alphabet, so the key needs quotes"
    );
    assert_eq!(
        ConfigPath::new(["say \"hi\""]).render(),
        "\"say \\\"hi\\\"\"",
        "a quote inside a quoted key must be escaped"
    );
    assert_eq!(
        ConfigPath::new(["a_b-c9"]).render(),
        "a_b-c9",
        "the bare alphabet must stay unquoted"
    );
}

#[test]
fn parts_round_trip_through_into_parts() {
    let program = Some(String::from("demo"));
    let missing = vec![leaf(&["url"], Vec::new())];
    let report = MissingRequiredValues::try_new(program.clone(), missing.clone())
        .expect("one entry forms a non-empty report");
    assert_eq!(report.program(), Some("demo"));
    assert_eq!(report.len(), 1);
    assert!(!report.is_empty());
    assert_eq!(report.into_parts(), (program, missing));
}
