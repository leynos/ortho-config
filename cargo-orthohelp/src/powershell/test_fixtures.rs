//! Shared test fixtures for `PowerShell` generator unit tests.

use crate::ir::{LocalizedDocMetadata, LocalizedHeadings, LocalizedSectionsMetadata};

pub(super) fn minimal_doc(locale: &str, about: &str) -> LocalizedDocMetadata {
    LocalizedDocMetadata {
        ir_version: "1.1".to_owned(),
        locale: locale.to_owned(),
        app_name: "fixture".to_owned(),
        bin_name: None,
        about: about.to_owned(),
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
        fields: vec![],
        subcommands: vec![],
        windows: None,
    }
}

pub(super) fn minimal_doc_with_subcommand() -> LocalizedDocMetadata {
    let mut root = minimal_doc("en-US", "Fixture");
    root.subcommands.push(LocalizedDocMetadata {
        app_name: "greet".to_owned(),
        about: "Greet".to_owned(),
        ..minimal_doc("en-US", "Greet")
    });
    root
}
