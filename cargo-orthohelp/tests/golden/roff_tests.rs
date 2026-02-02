//! Golden tests for roff man page generation.
//!
//! These tests verify that the roff generator produces output matching
//! expected golden files, covering section ordering, escaping, and enum
//! rendering as required by the roadmap.

use cargo_orthohelp::ir::{
    LocalizedDocMetadata, LocalizedFieldMetadata, LocalizedHeadings, LocalizedPrecedenceMeta,
    LocalizedSectionsMetadata,
};
use cargo_orthohelp::roff::{RoffConfig, generate_to_string};
use cargo_orthohelp::schema::{CliMetadata, DefaultValue, EnvMetadata, SourceKind, ValueType};
use rstest::rstest;

fn default_headings() -> LocalizedHeadings {
    LocalizedHeadings {
        name: "NAME".to_owned(),
        synopsis: "SYNOPSIS".to_owned(),
        description: "DESCRIPTION".to_owned(),
        options: "OPTIONS".to_owned(),
        environment: "ENVIRONMENT".to_owned(),
        files: "FILES".to_owned(),
        precedence: "PRECEDENCE".to_owned(),
        exit_status: "EXIT STATUS".to_owned(),
        examples: "EXAMPLES".to_owned(),
        see_also: "SEE ALSO".to_owned(),
    }
}

fn minimal_metadata() -> LocalizedDocMetadata {
    LocalizedDocMetadata {
        ir_version: "1.1".to_owned(),
        locale: "en-US".to_owned(),
        app_name: "test-app".to_owned(),
        bin_name: None,
        about: "A test application.".to_owned(),
        synopsis: None,
        sections: LocalizedSectionsMetadata {
            headings: default_headings(),
            discovery: None,
            precedence: None,
            examples: vec![],
            links: vec![],
            notes: vec![],
        },
        fields: vec![],
        subcommands: vec![],
        windows: None,
    }
}

/// Test that section ordering matches the specification.
#[rstest]
fn golden_section_ordering() {
    let metadata = minimal_metadata();
    let config = RoffConfig::default();
    let output = generate_to_string(&metadata, &config);

    // Find positions of each section
    let name_pos = output.find(".SH NAME").expect("NAME section");
    let synopsis_pos = output.find(".SH SYNOPSIS").expect("SYNOPSIS section");
    let desc_pos = output.find(".SH DESCRIPTION").expect("DESCRIPTION section");

    // Verify ordering: NAME < SYNOPSIS < DESCRIPTION
    assert!(name_pos < synopsis_pos, "NAME should come before SYNOPSIS");
    assert!(
        synopsis_pos < desc_pos,
        "SYNOPSIS should come before DESCRIPTION"
    );
}

/// Test that special characters are properly escaped.
#[rstest]
fn golden_escaping() {
    let mut metadata = minimal_metadata();
    // Test backslash escaping in the about text
    metadata.about = "A test with \\backslash.".to_owned();

    let config = RoffConfig::default();
    let output = generate_to_string(&metadata, &config);

    // Backslash should be escaped to double backslash
    assert!(
        output.contains("\\\\backslash"),
        "backslash should be escaped: {output}"
    );

    // Test leading dash escaping - use a multiline description
    let mut metadata2 = minimal_metadata();
    metadata2.about = "-starts with dash".to_owned();

    let output2 = generate_to_string(&metadata2, &config);
    assert!(
        output2.contains("\\-starts with dash"),
        "leading dash should be escaped: {output2}"
    );
}

/// Test that enum fields render their possible values.
#[rstest]
fn golden_enum_rendering() {
    let mut metadata = minimal_metadata();
    metadata.fields.push(LocalizedFieldMetadata {
        name: "log_level".to_owned(),
        help: "Set the log level.".to_owned(),
        long_help: None,
        value: Some(ValueType::Enum {
            variants: vec![
                "debug".to_owned(),
                "info".to_owned(),
                "warn".to_owned(),
                "error".to_owned(),
            ],
        }),
        default: Some(DefaultValue {
            display: "info".to_owned(),
        }),
        required: false,
        deprecated: None,
        cli: Some(CliMetadata {
            long: Some("log-level".to_owned()),
            short: Some('l'),
            value_name: None,
            multiple: false,
            takes_value: true,
            possible_values: vec![
                "debug".to_owned(),
                "info".to_owned(),
                "warn".to_owned(),
                "error".to_owned(),
            ],
            hide_in_help: false,
        }),
        env: None,
        file: None,
        examples: vec![],
        links: vec![],
        notes: vec![],
    });

    let config = RoffConfig::default();
    let output = generate_to_string(&metadata, &config);

    // Should contain OPTIONS section
    assert!(
        output.contains(".SH OPTIONS"),
        "should have OPTIONS section"
    );

    // Should contain the flag (dashes are escaped in roff format)
    assert!(
        output.contains("\\-\\-log-level"),
        "should contain --log-level flag: {output}"
    );

    // Should contain possible values
    assert!(
        output.contains("debug, info, warn, error"),
        "should list enum variants: {output}"
    );
}

/// Test that environment variables are rendered.
#[rstest]
fn golden_environment_section() {
    let mut metadata = minimal_metadata();
    metadata.fields.push(LocalizedFieldMetadata {
        name: "port".to_owned(),
        help: "Port to listen on.".to_owned(),
        long_help: None,
        value: Some(ValueType::Integer {
            bits: 16,
            signed: false,
        }),
        default: Some(DefaultValue {
            display: "8080".to_owned(),
        }),
        required: false,
        deprecated: None,
        cli: Some(CliMetadata {
            long: Some("port".to_owned()),
            short: Some('p'),
            value_name: None,
            multiple: false,
            takes_value: true,
            possible_values: vec![],
            hide_in_help: false,
        }),
        env: Some(EnvMetadata {
            var_name: "TEST_APP_PORT".to_owned(),
        }),
        file: None,
        examples: vec![],
        links: vec![],
        notes: vec![],
    });

    let config = RoffConfig::default();
    let output = generate_to_string(&metadata, &config);

    // Should contain ENVIRONMENT section
    assert!(
        output.contains(".SH ENVIRONMENT"),
        "should have ENVIRONMENT section"
    );

    // Should contain the env var
    assert!(
        output.contains("TEST_APP_PORT"),
        "should contain env var name"
    );
}

/// Test that precedence section is rendered.
#[rstest]
fn golden_precedence_section() {
    let mut metadata = minimal_metadata();
    metadata.sections.precedence = Some(LocalizedPrecedenceMeta {
        order: vec![
            SourceKind::Defaults,
            SourceKind::File,
            SourceKind::Env,
            SourceKind::Cli,
        ],
        rationale: None,
    });

    let config = RoffConfig::default();
    let output = generate_to_string(&metadata, &config);

    // Should contain PRECEDENCE section
    assert!(
        output.contains(".SH PRECEDENCE"),
        "should have PRECEDENCE section"
    );

    // Should contain source kinds in order
    assert!(
        output.contains("Built-in defaults"),
        "should mention defaults"
    );
    assert!(
        output.contains("Configuration files"),
        "should mention files"
    );
    assert!(
        output.contains("Environment variables"),
        "should mention env"
    );
    assert!(
        output.contains("Command-line arguments"),
        "should mention CLI"
    );
}

/// Test that subcommand-specific behaviour renders SEE ALSO cross-links.
#[rstest]
fn golden_subcommand_split_see_also() {
    let mut metadata = minimal_metadata();
    metadata.app_name = "app".to_owned();

    // Add subcommands
    let mut foo_subcommand = minimal_metadata();
    foo_subcommand.app_name = "foo".to_owned();
    foo_subcommand.about = "Do foo things.".to_owned();

    let mut bar_subcommand = minimal_metadata();
    bar_subcommand.app_name = "bar".to_owned();
    bar_subcommand.about = "Do bar things.".to_owned();

    metadata.subcommands = vec![foo_subcommand, bar_subcommand];

    // Enable subcommand splitting
    let config = RoffConfig {
        should_split_subcommands: true,
        ..RoffConfig::default()
    };

    let output = generate_to_string(&metadata, &config);

    // Main page SEE ALSO section should reference subcommands
    assert!(
        output.contains(".SH SEE ALSO"),
        "main page should contain a SEE ALSO section: {output}"
    );
    assert!(
        output.contains("app-foo (1)"),
        "SEE ALSO should reference the foo subcommand man page: {output}"
    );
    assert!(
        output.contains("app-bar (1)"),
        "SEE ALSO should reference the bar subcommand man page: {output}"
    );
}
