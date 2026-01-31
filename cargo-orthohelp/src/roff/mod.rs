//! Roff man page generator for `cargo-orthohelp`.
//!
//! Generates UNIX man pages from localised documentation metadata using
//! classic man macros (`.TH`, `.SH`, `.SS`, `.TP`, `.B`, `.I`).

#![allow(
    clippy::too_many_lines,
    reason = "roff generator builds content incrementally"
)]
#![allow(clippy::if_not_else, reason = "explicit structure clarifies branching")]
#![allow(
    clippy::shadow_unrelated,
    reason = "loop iteration rebinds path naturally"
)]
#![allow(
    clippy::format_push_string,
    reason = "roff templating uses format! for clarity"
)]
#![allow(
    clippy::expect_used,
    reason = "precondition filters guarantee presence"
)]

pub mod escape;
mod sections;
mod types;
mod writer;

pub use types::{RoffConfig, RoffOutput};

use crate::error::OrthohelpError;
use crate::ir::LocalizedDocMetadata;

/// Generates roff man page(s) from localised documentation metadata.
///
/// # Parameters
///
/// - `metadata`: The localised documentation IR to render.
/// - `config`: Generator configuration (section, date, output paths).
///
/// # Returns
///
/// A `RoffOutput` containing paths to all generated man page files.
///
/// # Errors
///
/// Returns `OrthohelpError::Io` if file creation fails.
pub fn generate(
    metadata: &LocalizedDocMetadata,
    config: &RoffConfig,
) -> Result<RoffOutput, OrthohelpError> {
    let mut output = RoffOutput::new();

    // Generate the main man page
    let content = generate_man_page(metadata, config);
    let bin_name = metadata.bin_name.as_deref().unwrap_or(&metadata.app_name);
    let info = writer::ManPageInfo::new(bin_name, config.section);
    let path = writer::write_man_page(&config.out_dir, &info, &content)?;
    output.add_file(path);

    // Handle subcommands
    if config.split_subcommands {
        for subcommand in &metadata.subcommands {
            let sub_content = generate_man_page(subcommand, config);
            let sub_name = subcommand
                .bin_name
                .as_deref()
                .unwrap_or(&subcommand.app_name);
            let sub_info = writer::ManPageInfo::with_subcommand(bin_name, sub_name, config.section);
            let path = writer::write_man_page(&config.out_dir, &sub_info, &sub_content)?;
            output.add_file(path);
        }
    }

    Ok(output)
}

fn generate_man_page(metadata: &LocalizedDocMetadata, config: &RoffConfig) -> String {
    let mut content = String::with_capacity(4096);
    let bin_name = metadata.bin_name.as_deref().unwrap_or(&metadata.app_name);
    let headings = &metadata.sections.headings;

    // Title header
    content.push_str(&sections::title_header(
        bin_name,
        config.section,
        config.date.as_deref(),
        config.source.as_deref(),
        config.manual.as_deref(),
    ));

    append_standard_sections(&mut content, metadata, bin_name, headings, config);
    append_inline_subcommands(&mut content, metadata, config);

    content
}

#[expect(clippy::too_many_arguments, reason = "helper groups related section calls")]
fn append_standard_sections(
    content: &mut String,
    metadata: &LocalizedDocMetadata,
    bin_name: &str,
    headings: &crate::ir::LocalizedHeadings,
    config: &RoffConfig,
) {
    // NAME section
    content.push_str(&sections::name_section(headings, bin_name, &metadata.about));

    // SYNOPSIS section
    content.push_str(&sections::synopsis_section(
        headings,
        bin_name,
        metadata.synopsis.as_deref(),
        &metadata.fields,
    ));

    // DESCRIPTION section
    content.push_str(&sections::description_section(headings, &metadata.about));

    // OPTIONS section
    content.push_str(&sections::options_section(headings, &metadata.fields));

    // ENVIRONMENT section
    content.push_str(&sections::environment_section(headings, &metadata.fields));

    // FILES section
    content.push_str(&sections::files_section(
        headings,
        &metadata.fields,
        metadata.sections.discovery.as_ref(),
    ));

    // PRECEDENCE section
    content.push_str(&sections::precedence_section(
        headings,
        metadata.sections.precedence.as_ref(),
    ));

    // EXAMPLES section
    content.push_str(&sections::examples_section(
        headings,
        &metadata.sections.examples,
    ));

    // SEE ALSO section
    let related_commands = collect_related_commands(metadata, bin_name, config);
    content.push_str(&sections::see_also_section(
        headings,
        &metadata.sections.links,
        &related_commands,
    ));

    // EXIT STATUS section
    content.push_str(&sections::exit_status_section(headings));
}

fn collect_related_commands(
    metadata: &LocalizedDocMetadata,
    bin_name: &str,
    config: &RoffConfig,
) -> Vec<String> {
    if !config.split_subcommands {
        return Vec::new();
    }

    metadata
        .subcommands
        .iter()
        .map(|s| {
            let sub_name = s.bin_name.as_deref().unwrap_or(&s.app_name);
            format!("{bin_name}-{sub_name}")
        })
        .collect()
}

fn append_inline_subcommands(
    content: &mut String,
    metadata: &LocalizedDocMetadata,
    config: &RoffConfig,
) {
    if config.split_subcommands || metadata.subcommands.is_empty() {
        return;
    }

    content.push_str(".SH COMMANDS\n");
    for subcommand in &metadata.subcommands {
        content.push_str(&generate_subcommand_section(subcommand));
    }
}

fn generate_subcommand_section(metadata: &LocalizedDocMetadata) -> String {
    let mut content = String::new();
    let name = metadata.bin_name.as_deref().unwrap_or(&metadata.app_name);

    content.push_str(&format!(".SS {name}\n"));
    content.push_str(&escape::escape_text(&metadata.about));
    content.push('\n');

    // Include options for the subcommand
    let cli_fields: Vec<_> = metadata
        .fields
        .iter()
        .filter(|f| f.cli.as_ref().is_some_and(|c| !c.hide_in_help))
        .collect();

    if !cli_fields.is_empty() {
        content.push_str(".PP\nOptions:\n");
        for field in cli_fields {
            let cli = field.cli.as_ref().expect("filtered");
            content.push_str(".TP\n");

            let flag_line = if cli.takes_value {
                let value_name = cli
                    .value_name
                    .as_deref()
                    .or_else(|| field.value.as_ref().map(escape::value_type_placeholder))
                    .unwrap_or("VALUE");
                escape::format_flag_with_value(cli.long.as_deref(), cli.short, value_name)
            } else {
                escape::format_flag(cli.long.as_deref(), cli.short)
            };
            content.push_str(&flag_line);
            content.push('\n');
            content.push_str(&escape::escape_text(&field.help));
            content.push('\n');
        }
    }

    content
}

/// Generates man page content as a string without writing to disk.
///
/// Useful for testing and golden file generation.
#[must_use]
pub fn generate_to_string(metadata: &LocalizedDocMetadata, config: &RoffConfig) -> String {
    generate_man_page(metadata, config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{LocalizedHeadings, LocalizedSectionsMetadata};

    fn minimal_metadata() -> LocalizedDocMetadata {
        LocalizedDocMetadata {
            ir_version: "1.1".to_owned(),
            locale: "en-US".to_owned(),
            app_name: "test-app".to_owned(),
            bin_name: None,
            about: "A test application.".to_owned(),
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

    #[test]
    fn generate_to_string_produces_valid_roff() {
        let metadata = minimal_metadata();
        let config = RoffConfig::default();
        let result = generate_to_string(&metadata, &config);

        assert!(result.starts_with(".TH \"TEST-APP\" \"1\""));
        assert!(result.contains(".SH NAME"));
        assert!(result.contains("test-app \\- A test application."));
        assert!(result.contains(".SH SYNOPSIS"));
        assert!(result.contains(".SH DESCRIPTION"));
    }
}
