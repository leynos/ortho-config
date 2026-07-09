//! Roff man page generator for `cargo-orthohelp`.
//!
//! Generates UNIX man pages from localized documentation metadata using
//! classic man macros (`.TH`, `.SH`, `.SS`, `.TP`, `.B`, `.I`).

mod entry;
pub mod escape;
mod sections;
mod types;
mod writer;

pub use types::{InvalidManSection, ManSection, RoffConfig, RoffOutput};

use crate::error::OrthohelpError;
use crate::ir::LocalizedDocMetadata;

/// Generates roff man page(s) from localized documentation metadata.
///
/// # Parameters
///
/// - `metadata`: The localized documentation IR to render.
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
    let main_path = writer::write_man_page(&config.out_dir, &info, &content)?;
    output.add_file(main_path);

    // Handle subcommands
    if config.should_split_subcommands {
        for subcommand in &metadata.subcommands {
            let sub_name = subcommand
                .bin_name
                .as_deref()
                .unwrap_or(&subcommand.app_name);
            let composite_name = format!("{bin_name}-{sub_name}");
            let sub_content = generate_subcommand_page(subcommand, config, &composite_name);
            let sub_info = writer::ManPageInfo::with_subcommand(bin_name, sub_name, config.section);
            let sub_path = writer::write_man_page(&config.out_dir, &sub_info, &sub_content)?;
            output.add_file(sub_path);
        }
    }

    Ok(output)
}

fn generate_man_page(metadata: &LocalizedDocMetadata, config: &RoffConfig) -> String {
    let bin_name = metadata.bin_name.as_deref().unwrap_or(&metadata.app_name);
    generate_man_page_with_name(metadata, config, bin_name)
}

fn generate_subcommand_page(
    metadata: &LocalizedDocMetadata,
    config: &RoffConfig,
    composite_name: &str,
) -> String {
    let mut subcommand_config = config.clone();
    subcommand_config.should_split_subcommands = false;
    generate_man_page_with_name(metadata, &subcommand_config, composite_name)
}

fn generate_man_page_with_name(
    metadata: &LocalizedDocMetadata,
    config: &RoffConfig,
    display_name: &str,
) -> String {
    let mut content = String::with_capacity(4096);

    // Title header
    let title_meta = sections::TitleMetadata::new(
        config.date.as_deref(),
        config.source.as_deref(),
        config.manual.as_deref(),
    );
    content.push_str(&sections::title_header(
        display_name,
        config.section,
        &title_meta,
    ));

    append_standard_sections(&mut content, metadata, config, display_name);
    append_inline_subcommands(&mut content, metadata, config);

    content
}

fn append_standard_sections(
    content: &mut String,
    metadata: &LocalizedDocMetadata,
    config: &RoffConfig,
    display_name: &str,
) {
    let headings = &metadata.sections.headings;
    // NAME section
    content.push_str(&sections::name_section(
        headings,
        display_name,
        &metadata.about,
    ));

    // SYNOPSIS section
    content.push_str(&sections::synopsis_section(
        headings,
        display_name,
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
    let related_commands = collect_related_commands(metadata, display_name, config);
    content.push_str(&sections::see_also_section(
        headings,
        &metadata.sections.links,
        &related_commands,
        config.section,
    ));

    // EXIT STATUS section
    content.push_str(&sections::exit_status_section(headings));
}

fn collect_related_commands(
    metadata: &LocalizedDocMetadata,
    bin_name: &str,
    config: &RoffConfig,
) -> Vec<String> {
    if !config.should_split_subcommands {
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
    if config.should_split_subcommands || metadata.subcommands.is_empty() {
        return;
    }

    content.push_str(".SH ");
    content.push_str(&escape::escape_macro_arg(
        &metadata.sections.headings.commands,
    ));
    content.push('\n');
    for subcommand in &metadata.subcommands {
        content.push_str(&generate_subcommand_section(subcommand));
    }
}

fn generate_subcommand_section(metadata: &LocalizedDocMetadata) -> String {
    let mut content = String::new();
    let name = metadata.bin_name.as_deref().unwrap_or(&metadata.app_name);

    content.push_str(".SS ");
    content.push_str(&escape::escape_macro_arg(name));
    content.push('\n');
    content.push_str(&escape::escape_text(&metadata.about));
    content.push('\n');

    // Include options for the subcommand
    let cli_fields: Vec<_> = metadata
        .fields
        .iter()
        .filter_map(|f| f.cli.as_ref().filter(|c| !c.hide_in_help).map(|c| (f, c)))
        .collect();

    if !cli_fields.is_empty() {
        content.push_str(".PP\n");
        content.push_str(&escape::escape_text(&metadata.sections.headings.options));
        content.push_str(":\n");
        for (field, cli) in cli_fields {
            content.push_str(".TP\n");

            let placeholder = field.value.as_ref().map(escape::value_type_placeholder);
            let flag_line = if cli.takes_value {
                let value_name = cli
                    .value_name
                    .as_deref()
                    .or(placeholder.as_deref())
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
    //! Unit tests for man page generation.

    use super::*;
    use crate::ir::{LocalizedHeadings, LocalizedSectionsMetadata};
    use crate::test_support::nested_fixture::nested_doc;
    use camino::Utf8PathBuf;
    use cap_std::ambient_authority;
    use cap_std::fs_utf8::Dir;
    use std::io::Read;

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

    #[test]
    fn generate_to_string_includes_inline_subcommands() {
        let mut metadata = minimal_metadata();
        metadata.subcommands = vec![
            LocalizedDocMetadata {
                app_name: "run".to_owned(),
                about: "Run the task.".to_owned(),
                ..minimal_metadata()
            },
            LocalizedDocMetadata {
                app_name: "audit".to_owned(),
                about: "Audit the task.".to_owned(),
                ..minimal_metadata()
            },
        ];
        let config = RoffConfig::default();
        let result = generate_to_string(&metadata, &config);

        assert!(result.contains(".SH COMMANDS"));
        assert!(result.contains(".SS run"));
        assert!(result.contains("Run the task."));
        assert!(result.contains(".SS audit"));
        assert!(result.contains("Audit the task."));
    }

    #[test]
    fn inline_subcommands_render_for_nested_fixture() {
        let metadata = nested_doc();
        let config = RoffConfig::default();
        let result = generate_to_string(&metadata, &config);

        assert!(result.contains(".SH COMMANDS"));
        assert!(result.contains(".SS greet"));
        assert!(result.contains(".SS version"));
        assert!(result.contains(".SS admin"));
        assert!(result.contains("\\fB\\-\\-recipient\\fR \\fIRECIPIENT\\fR"));
    }

    #[test]
    fn split_subcommands_render_for_nested_fixture() {
        let metadata = nested_doc();
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let out_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
            .expect("temp dir path should be UTF-8");
        let config = RoffConfig {
            out_dir: out_dir.clone(),
            date: Some("2026-06-04".to_owned()),
            should_split_subcommands: true,
            ..RoffConfig::default()
        };
        let output = generate(&metadata, &config).expect("generate split man pages");
        let filenames = output
            .files
            .iter()
            .filter_map(|path| path.file_name())
            .collect::<Vec<_>>();

        assert!(filenames.contains(&"fixture-greet.1"));
        assert!(filenames.contains(&"fixture-version.1"));
        assert!(filenames.contains(&"fixture-admin.1"));

        let admin_page =
            read_utf8(&out_dir, "man/man1/fixture-admin.1").expect("read generated man page");
        assert!(admin_page.contains(".SH COMMANDS"));
        assert!(admin_page.contains(".SS audit"));
        assert!(admin_page.contains(".SS grant-access"));
    }

    fn read_utf8(out_dir: &Utf8PathBuf, relative: &str) -> std::io::Result<String> {
        let dir = Dir::open_ambient_dir(out_dir, ambient_authority())?;
        let mut file = dir.open(relative)?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;
        Ok(content)
    }
}
