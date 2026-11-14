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
