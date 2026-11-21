//! Presentation tests for the missing-required-values report.
//!
//! The payload is constructed directly here so display is exercised without
//! the collector. Ordering, path quoting, route grouping, and the object-shaped
//! file route are display contracts that must not depend on discovery.
//!
//! Membership and classification use `googletest` matchers, exact ordered and
//! multiline output uses `pretty_assertions`, and the full English rendering is
//! pinned by `insta` so incidental prose changes are reviewed rather than
//! silently accepted.
//!
//! No localized presentation path is exposed for this error: the payload is
//! structured data and translation belongs to the consumer that renders it.
//! These tests therefore pin the English rendering only.

use anyhow::{Result, anyhow};
use googletest::prelude::*;
use insta::assert_snapshot;
use ortho_config::{ConfigPath, MissingRequiredValue, MissingRequiredValues, SupplyRoute};
use pretty_assertions::assert_eq;

/// Build a command-line route.
fn cli(flag: &str, takes_value: bool) -> SupplyRoute {
    SupplyRoute::CommandLine {
        flag: String::from(flag),
        takes_value,
    }
}

/// Build an environment-variable route.
fn env(variable: &str) -> SupplyRoute {
    SupplyRoute::Environment {
        variable: String::from(variable),
    }
}

/// Build a configuration-file route from path segments.
fn file(segments: &[&str], expects_object: bool) -> SupplyRoute {
    SupplyRoute::ConfigurationFile {
        path: ConfigPath::new(segments.iter().copied()),
        expects_object,
    }
}

/// Build one missing requirement at `segments`.
fn missing(
    segments: &[&str],
    routes: Vec<SupplyRoute>,
    expects_object: bool,
) -> MissingRequiredValue {
    MissingRequiredValue::new(
        ConfigPath::new(segments.iter().copied()),
        routes,
        expects_object,
    )
}

/// Build a report labelled `program`.
///
/// # Errors
///
/// Returns an error when `items` is empty, because the payload rejects an empty
/// report and every fixture here supplies at least one entry.
fn report_of(
    program: Option<&str>,
    items: Vec<MissingRequiredValue>,
) -> Result<MissingRequiredValues> {
    MissingRequiredValues::try_new(program.map(String::from), items)
        .ok_or_else(|| anyhow!("the fixture must report at least one missing value"))
}

/// Render every item path in report order.
fn paths(report: &MissingRequiredValues) -> Vec<String> {
    report
        .items()
        .iter()
        .map(|item| item.path().render())
        .collect()
}

/// A report exercising every supply-route section and both file key shapes.
fn mixed_report() -> Result<MissingRequiredValues> {
    report_of(
        Some("demo"),
        vec![
            missing(
                &["reference"],
                vec![
                    cli("--reference", true),
                    env("MRQ_REFERENCE"),
                    file(&["reference"], false),
                ],
                false,
            ),
            missing(
                &["database"],
                vec![env("MRN_DATABASE"), file(&["database"], true)],
                true,
            ),
            missing(
                &["database", "host"],
                vec![
                    env("MRN_DATABASE__HOST"),
                    file(&["database", "host"], false),
                ],
                false,
            ),
        ],
    )
}

#[test]
fn a_mixed_report_renders_every_route_section() -> Result<()> {
    let report = mixed_report()?;

    assert_that!(report.program(), eq(Some("demo")));
    assert_that!(report.len(), eq(3));
    assert_that!(
        report
            .items()
            .last()
            .map(MissingRequiredValue::expects_object),
        eq(Some(false))
    );
    assert_eq!(
        paths(&report),
        vec![
            String::from("reference"),
            String::from("database"),
            String::from("database.host"),
        ]
    );

    let rendered = report.to_string();
    assert_that!(&rendered, contains_substring("--reference <value>"));
    assert_that!(&rendered, contains_substring("MRN_DATABASE__HOST=<value>"));
    assert_that!(&rendered, contains_substring("[database]"));
    assert_snapshot!(rendered);
    Ok(())
}

#[test]
fn an_object_requirement_uses_bracket_syntax() -> Result<()> {
    let report = report_of(
        None,
        vec![missing(
            &["database"],
            vec![file(&["database"], true)],
            true,
        )],
    )?;

    let rendered = report.to_string();
    assert_that!(&rendered, contains_substring("\n     [database]\n"));
    assert_that!(&rendered, not(contains_substring("= \"<value>\"")));
    Ok(())
}

#[test]
fn a_leaf_nested_in_a_table_renders_both_lines() -> Result<()> {
    let report = report_of(
        None,
        vec![missing(
            &["server", "port"],
            vec![file(&["server", "port"], false)],
            false,
        )],
    )?;

    let expected = concat!(
        "Missing required configuration\n",
        "\n",
        "The following value(s) are missing:\n",
        "  - `server.port`\n",
        "\n",
        "Please provide them using one of the following methods, in order of precedence:\n",
        "  1. Configuration File:\n",
        "     [server]\n",
        "     port = \"<value>\"\n",
    );
    assert_eq!(report.to_string(), expected);
    Ok(())
}

#[test]
fn quoting_keeps_a_literal_dotted_key_distinct() -> Result<()> {
    let report = report_of(
        None,
        vec![missing(
            &["server", "api.key"],
            vec![file(&["server", "api.key"], false)],
            false,
        )],
    )?;

    assert_eq!(paths(&report), vec![String::from("server.\"api.key\"")]);
    assert_that!(
        &report.to_string(),
        contains_substring("\"api.key\" = \"<value>\"")
    );
    Ok(())
}

#[test]
fn segments_with_whitespace_or_no_content_are_quoted() -> Result<()> {
    let report = report_of(
        None,
        vec![
            missing(&["display name"], Vec::new(), false),
            missing(&[""], Vec::new(), false),
        ],
    )?;

    assert_eq!(
        paths(&report),
        vec![String::from("\"display name\""), String::from("\"\"")]
    );
    Ok(())
}

#[test]
fn numbering_counts_only_the_sections_that_are_present() -> Result<()> {
    let report = report_of(
        None,
        vec![missing(&["reference"], vec![env("MRQ_REFERENCE")], false)],
    )?;

    let rendered = report.to_string();
    assert_that!(&rendered, contains_substring("  1. Environment Variable:"));
    assert_that!(&rendered, not(contains_substring("Command-Line Argument")));
    assert_that!(&rendered, not(contains_substring("Configuration File")));
    assert_that!(&rendered, not(contains_substring("2.")));
    Ok(())
}

#[test]
fn a_report_without_routes_omits_the_recovery_prompt() -> Result<()> {
    let report = report_of(None, vec![missing(&["secret"], Vec::new(), false)])?;

    let expected = concat!(
        "Missing required configuration\n",
        "\n",
        "The following value(s) are missing:\n",
        "  - `secret`\n",
    );
    assert_eq!(report.to_string(), expected);
    assert_that!(
        &report.to_string(),
        not(contains_substring("Please provide them"))
    );
    Ok(())
}

#[test]
fn a_report_without_a_program_label_uses_the_generic_heading() -> Result<()> {
    let report = report_of(None, vec![missing(&["secret"], Vec::new(), false)])?;

    assert_that!(report.program(), eq(None));
    assert_that!(
        &report.to_string(),
        contains_substring("Missing required configuration\n")
    );
    Ok(())
}

#[test]
fn an_empty_report_is_rejected_at_construction() {
    assert_eq!(
        MissingRequiredValues::try_new(None, std::iter::empty()),
        None
    );
}

#[test]
fn display_never_contains_a_supplied_value() -> Result<()> {
    // The payload has no value slots at all, so the rendered report cannot leak
    // one even for a secret requirement.
    let report = report_of(
        Some("vault"),
        vec![missing(
            &["api_token"],
            vec![cli("--api-token", true), env("VAULT_API_TOKEN")],
            false,
        )],
    )?;

    let rendered = report.to_string();
    assert_that!(&rendered, not(contains_substring("s3cret")));
    assert_that!(&rendered, contains_substring("<value>"));
    Ok(())
}
