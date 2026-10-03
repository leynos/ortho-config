//! Shared fixture metadata for `PowerShell` golden tests.

use cargo_orthohelp::ir::{LocalizedDocMetadata, LocalizedHeadings, LocalizedSectionsMetadata};
use cargo_orthohelp::schema::{CliMetadata, DefaultValue, EnvMetadata, FileMetadata, ValueType};

/// Builds minimal localized metadata used by `PowerShell` golden tests.
pub(crate) fn minimal_doc() -> LocalizedDocMetadata {
    LocalizedDocMetadata {
        ir_version: "1.1".to_owned(),
        locale: "en-US".to_owned(),
        app_name: "fixture".to_owned(),
        bin_name: None,
        about: "Fixture app".to_owned(),
        synopsis: None,
        sections: LocalizedSectionsMetadata {
            headings: LocalizedHeadings {
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
                commands: "COMMANDS".to_owned(),
            },
            discovery: None,
            precedence: None,
            examples: vec![],
            links: vec![],
            notes: vec![],
        },
        fields: vec![cargo_orthohelp::ir::LocalizedFieldMetadata {
            name: "port".to_owned(),
            help: "Port used by the fixture service.".to_owned(),
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
                var_name: "FIXTURE_PORT".to_owned(),
            }),
            file: Some(FileMetadata {
                key_path: "server.port".to_owned(),
            }),
            examples: vec![],
            links: vec![],
            notes: vec![],
        }],
        subcommands: vec![],
        windows: None,
    }
}
