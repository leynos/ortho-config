//! Tests that keep the local schema aligned with `ortho_config::docs`.

use super::*;
use ortho_config::docs as ortho_docs;

#[test]
fn ir_version_matches_ortho_config() {
    assert_eq!(
        ORTHO_DOCS_IR_VERSION,
        ortho_docs::ORTHO_DOCS_IR_VERSION,
        "IR schema version should match ortho_config::docs"
    );
}

#[test]
fn schema_round_trips_against_ortho_config() {
    let source = sample_metadata();
    let as_value = serde_json::to_value(&source).expect("serialize ortho_config metadata");
    let parsed: DocMetadata =
        serde_json::from_value(as_value.clone()).expect("deserialize into local schema");
    let round_trip = serde_json::to_value(parsed).expect("serialize local schema back to JSON");
    assert_eq!(
        as_value, round_trip,
        "schema JSON should stay compatible with ortho_config::docs"
    );
}

fn sample_metadata() -> ortho_docs::DocMetadata {
    ortho_docs::DocMetadata {
        ir_version: ORTHO_DOCS_IR_VERSION.to_owned(),
        app_name: "demo-app".to_owned(),
        bin_name: Some("demo-app".to_owned()),
        about_id: "demo.about".to_owned(),
        synopsis_id: Some("demo.synopsis".to_owned()),
        sections: sample_sections(),
        fields: vec![sample_field()],
        subcommands: Vec::new(),
        windows: Some(sample_windows()),
    }
}

fn sample_sections() -> ortho_docs::SectionsMetadata {
    ortho_docs::SectionsMetadata {
        headings_ids: sample_headings_ids(),
        discovery: Some(sample_discovery()),
        precedence: Some(sample_precedence()),
        examples: vec![sample_example()],
        links: vec![sample_link()],
        notes: vec![sample_note()],
    }
}

fn sample_headings_ids() -> ortho_docs::HeadingIds {
    ortho_docs::HeadingIds {
        name: "demo.headings.name".to_owned(),
        synopsis: "demo.headings.synopsis".to_owned(),
        description: "demo.headings.description".to_owned(),
        options: "demo.headings.options".to_owned(),
        environment: "demo.headings.environment".to_owned(),
        files: "demo.headings.files".to_owned(),
        precedence: "demo.headings.precedence".to_owned(),
        exit_status: "demo.headings.exit_status".to_owned(),
        examples: "demo.headings.examples".to_owned(),
        see_also: "demo.headings.see_also".to_owned(),
        commands: Some("demo.headings.commands".to_owned()),
    }
}

fn sample_discovery() -> ortho_docs::ConfigDiscoveryMeta {
    ortho_docs::ConfigDiscoveryMeta {
        formats: vec![
            ortho_docs::ConfigFormat::Toml,
            ortho_docs::ConfigFormat::Yaml,
            ortho_docs::ConfigFormat::Json,
        ],
        search_paths: vec![ortho_docs::PathPattern {
            pattern: "config.{toml,yaml}".to_owned(),
            note_id: Some("demo.discovery.note".to_owned()),
        }],
        override_flag_long: Some("config".to_owned()),
        override_env: Some("DEMO_CONFIG".to_owned()),
        xdg_compliant: true,
    }
}

fn sample_precedence() -> ortho_docs::PrecedenceMeta {
    ortho_docs::PrecedenceMeta {
        order: vec![
            ortho_docs::SourceKind::Defaults,
            ortho_docs::SourceKind::File,
            ortho_docs::SourceKind::Env,
            ortho_docs::SourceKind::Cli,
        ],
        rationale_id: Some("demo.precedence".to_owned()),
    }
}

fn sample_example() -> ortho_docs::Example {
    ortho_docs::Example {
        title_id: Some("demo.example.title".to_owned()),
        code: "demo --help".to_owned(),
        body_id: None,
    }
}

fn sample_link() -> ortho_docs::Link {
    ortho_docs::Link {
        text_id: Some("demo.link".to_owned()),
        uri: "https://example.com".to_owned(),
    }
}

fn sample_note() -> ortho_docs::Note {
    ortho_docs::Note {
        text_id: "demo.note".to_owned(),
    }
}

fn sample_field() -> ortho_docs::FieldMetadata {
    ortho_docs::FieldMetadata {
        name: "port".to_owned(),
        help_id: "demo.fields.port.help".to_owned(),
        long_help_id: Some("demo.fields.port.long_help".to_owned()),
        value: Some(ortho_docs::ValueType::Integer {
            bits: 16,
            signed: false,
        }),
        default: Some(ortho_docs::DefaultValue {
            display: "8080".to_owned(),
        }),
        required: true,
        deprecated: Some(ortho_docs::Deprecation {
            note_id: "demo.fields.port.deprecated".to_owned(),
        }),
        cli: Some(sample_cli_metadata()),
        env: Some(sample_env_metadata()),
        file: Some(sample_file_metadata()),
        examples: vec![sample_field_example()],
        links: vec![sample_field_link()],
        notes: vec![sample_field_note()],
    }
}

fn sample_cli_metadata() -> ortho_docs::CliMetadata {
    ortho_docs::CliMetadata {
        long: Some("port".to_owned()),
        short: Some('p'),
        value_name: Some("PORT".to_owned()),
        multiple: false,
        takes_value: true,
        possible_values: vec!["8080".to_owned(), "9090".to_owned()],
        hide_in_help: false,
    }
}

fn sample_env_metadata() -> ortho_docs::EnvMetadata {
    ortho_docs::EnvMetadata {
        var_name: "DEMO_PORT".to_owned(),
    }
}

fn sample_file_metadata() -> ortho_docs::FileMetadata {
    ortho_docs::FileMetadata {
        key_path: "network.port".to_owned(),
    }
}

fn sample_field_example() -> ortho_docs::Example {
    ortho_docs::Example {
        title_id: None,
        code: "demo --port 8080".to_owned(),
        body_id: Some("demo.example.body".to_owned()),
    }
}

fn sample_field_link() -> ortho_docs::Link {
    ortho_docs::Link {
        text_id: None,
        uri: "https://example.com/port".to_owned(),
    }
}

fn sample_field_note() -> ortho_docs::Note {
    ortho_docs::Note {
        text_id: "demo.fields.port.note".to_owned(),
    }
}

fn sample_windows() -> ortho_docs::WindowsMetadata {
    ortho_docs::WindowsMetadata {
        module_name: Some("Demo".to_owned()),
        export_aliases: vec!["demo".to_owned()],
        include_common_parameters: true,
        split_subcommands_into_functions: false,
        help_info_uri: Some("https://example.com/help".to_owned()),
    }
}
