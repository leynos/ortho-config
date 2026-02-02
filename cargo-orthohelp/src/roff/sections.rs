//! Section content generators for man pages.
//!
//! Provides functions to generate each standard man page section from
//! localised documentation metadata.

use crate::ir::{
    LocalizedConfigDiscoveryMeta, LocalizedExample, LocalizedFieldMetadata, LocalizedHeadings,
    LocalizedLink, LocalizedPrecedenceMeta,
};
use crate::schema::SourceKind;

use super::escape::{
    bold, escape_macro_arg, escape_text, format_flag, format_flag_with_value, italic,
};

/// Metadata for the man page title header.
pub struct TitleMetadata<'a> {
    /// Optional date string (e.g., "2026-01-31").
    pub date: Option<&'a str>,
    /// Optional source string (e.g., "v1.0").
    pub source: Option<&'a str>,
    /// Optional manual name (e.g., "User Commands").
    pub manual: Option<&'a str>,
}

impl<'a> TitleMetadata<'a> {
    /// Creates a new `TitleMetadata` with the given fields.
    #[must_use]
    pub const fn new(
        date: Option<&'a str>,
        source: Option<&'a str>,
        manual: Option<&'a str>,
    ) -> Self {
        Self {
            date,
            source,
            manual,
        }
    }
}

/// Generates the `.TH` title header macro.
///
/// Format: `.TH NAME SECTION DATE SOURCE MANUAL`
pub fn title_header(name: &str, section: u8, metadata: &TitleMetadata) -> String {
    let name_upper = escape_macro_arg(&name.to_uppercase());
    let date_str = metadata.date.map_or_else(String::new, escape_macro_arg);
    let source_str = metadata.source.map_or_else(String::new, escape_macro_arg);
    let manual_str = metadata.manual.map_or_else(String::new, escape_macro_arg);
    format!(".TH \"{name_upper}\" \"{section}\" \"{date_str}\" \"{source_str}\" \"{manual_str}\"\n")
}

/// Generates the NAME section content.
pub fn name_section(headings: &LocalizedHeadings, name: &str, about: &str) -> String {
    let escaped_about = escape_text(about);
    format!(".SH {}\n{name} \\- {escaped_about}\n", headings.name)
}

/// Generates the SYNOPSIS section content.
#[expect(
    clippy::excessive_nesting,
    reason = "field iteration with CLI filtering requires nested conditionals"
)]
pub fn synopsis_section(
    headings: &LocalizedHeadings,
    bin_name: &str,
    synopsis: Option<&str>,
    fields: &[LocalizedFieldMetadata],
) -> String {
    let mut output = format!(".SH {}\n.B {bin_name}\n", headings.synopsis);

    if let Some(syn) = synopsis {
        output.push_str(&escape_text(syn));
        output.push('\n');
    } else {
        // Generate synopsis from CLI fields
        for field in fields {
            if let Some(cli) = &field.cli {
                if cli.hide_in_help {
                    continue;
                }
                output.push_str(&format_synopsis_option(field, cli));
            }
        }
    }

    output
}

fn format_synopsis_option(
    field: &LocalizedFieldMetadata,
    cli: &crate::schema::CliMetadata,
) -> String {
    let flag = if cli.takes_value {
        let value_name = cli
            .value_name
            .as_deref()
            .or_else(|| {
                field
                    .value
                    .as_ref()
                    .map(super::escape::value_type_placeholder)
            })
            .unwrap_or("VALUE");
        format_flag_with_value(cli.long.as_deref(), cli.short, value_name)
    } else {
        format_flag(cli.long.as_deref(), cli.short)
    };

    if field.required {
        format!("{flag}\n")
    } else {
        format!("[{flag}]\n")
    }
}

/// Generates the DESCRIPTION section content.
pub fn description_section(headings: &LocalizedHeadings, about: &str) -> String {
    let escaped = escape_text(about);
    format!(".SH {}\n{escaped}\n", headings.description)
}

/// Generates the OPTIONS section content.
pub fn options_section(headings: &LocalizedHeadings, fields: &[LocalizedFieldMetadata]) -> String {
    let cli_fields: Vec<_> = fields
        .iter()
        .filter(|f| f.cli.as_ref().is_some_and(|c| !c.hide_in_help))
        .collect();

    if cli_fields.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", headings.options);

    for field in cli_fields {
        output.push_str(&format_option_entry(field));
    }

    output
}

#[expect(
    clippy::expect_used,
    reason = "precondition: field was filtered to have CLI metadata"
)]
#[expect(
    clippy::format_push_string,
    reason = "roff templating uses format! for clarity"
)]
fn format_option_entry(field: &LocalizedFieldMetadata) -> String {
    let cli = field.cli.as_ref().expect("filtered for CLI fields");
    let mut output = String::new();

    // Tag paragraph with flag
    output.push_str(".TP\n");

    let flag_line = if cli.takes_value {
        let value_name = cli
            .value_name
            .as_deref()
            .or_else(|| {
                field
                    .value
                    .as_ref()
                    .map(super::escape::value_type_placeholder)
            })
            .unwrap_or("VALUE");
        format_flag_with_value(cli.long.as_deref(), cli.short, value_name)
    } else {
        format_flag(cli.long.as_deref(), cli.short)
    };
    output.push_str(&flag_line);
    output.push('\n');

    // Help text
    let help = field.long_help.as_deref().unwrap_or(&field.help);
    output.push_str(&escape_text(help));
    output.push('\n');

    // Default value
    if let Some(default) = &field.default {
        output.push_str(".br\n");
        output.push_str(&format!("Default: {}\n", bold(&default.display)));
    }

    // Possible values for enums
    if !cli.possible_values.is_empty() {
        output.push_str(".br\n");
        let values = cli.possible_values.join(", ");
        output.push_str(&format!("Possible values: {}\n", italic(&values)));
    }

    // Deprecation notice
    if let Some(deprecated) = &field.deprecated {
        output.push_str(".br\n");
        output.push_str(&format!("DEPRECATED: {}\n", escape_text(&deprecated.note)));
    }

    output
}

/// Generates the ENVIRONMENT section content.
pub fn environment_section(
    headings: &LocalizedHeadings,
    fields: &[LocalizedFieldMetadata],
) -> String {
    let env_fields: Vec<_> = fields.iter().filter(|f| f.env.is_some()).collect();

    if env_fields.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", headings.environment);

    // Sort by environment variable name for consistency
    let mut sorted = env_fields;
    sorted.sort_by(|a, b| {
        a.env
            .as_ref()
            .map(|e| &e.var_name)
            .cmp(&b.env.as_ref().map(|e| &e.var_name))
    });

    for field in sorted {
        output.push_str(&format_env_entry(field));
    }

    output
}

#[expect(
    clippy::expect_used,
    reason = "precondition: field was filtered to have env metadata"
)]
#[expect(
    clippy::format_push_string,
    reason = "roff templating uses format! for clarity"
)]
fn format_env_entry(field: &LocalizedFieldMetadata) -> String {
    let env = field.env.as_ref().expect("filtered for env fields");
    let mut output = String::new();

    output.push_str(".TP\n");
    output.push_str(&bold(&env.var_name));
    output.push('\n');
    output.push_str(&escape_text(&field.help));

    // Cross-reference CLI flag if available
    if let Some(cli) = &field.cli {
        if let Some(long) = &cli.long {
            output.push_str(&format!(" Equivalent to {}.", bold(&format!("--{long}"))));
        }
    }
    output.push('\n');

    output
}

/// Generates the FILES section content.
pub fn files_section(
    headings: &LocalizedHeadings,
    fields: &[LocalizedFieldMetadata],
    discovery: Option<&LocalizedConfigDiscoveryMeta>,
) -> String {
    let file_fields: Vec<_> = fields.iter().filter(|f| f.file.is_some()).collect();
    let has_discovery = discovery.is_some_and(|d| !d.search_paths.is_empty());

    if file_fields.is_empty() && !has_discovery {
        return String::new();
    }

    let mut output = format!(".SH {}\n", headings.files);

    if let Some(disc) = discovery {
        render_discovery_section(&mut output, disc);
    }

    render_file_fields(&mut output, &file_fields);

    output
}

fn render_discovery_section(output: &mut String, disc: &LocalizedConfigDiscoveryMeta) {
    render_search_paths(output, disc);
    render_supported_formats(output, disc);
    render_xdg_compliance(output, disc);
}

fn render_search_paths(output: &mut String, disc: &LocalizedConfigDiscoveryMeta) {
    for path in &disc.search_paths {
        output.push_str(".TP\n");
        output.push_str(&italic(&path.pattern));
        output.push('\n');
        if let Some(note) = &path.note {
            output.push_str(&escape_text(note));
            output.push('\n');
        }
    }
}

#[expect(
    clippy::format_push_string,
    reason = "roff templating uses format! for clarity"
)]
fn render_supported_formats(output: &mut String, disc: &LocalizedConfigDiscoveryMeta) {
    if disc.formats.is_empty() {
        return;
    }

    let formats: Vec<_> = disc.formats.iter().map(format_config_format).collect();
    output.push_str(".PP\n");
    output.push_str(&format!("Supported formats: {}.\n", formats.join(", ")));
}

fn render_xdg_compliance(output: &mut String, disc: &LocalizedConfigDiscoveryMeta) {
    if disc.xdg_compliant {
        output.push_str(".PP\n");
        output.push_str("Configuration discovery follows the XDG Base Directory specification.\n");
    }
}

#[expect(
    clippy::expect_used,
    reason = "precondition: field was filtered to have file metadata"
)]
fn render_file_fields(output: &mut String, file_fields: &[&LocalizedFieldMetadata]) {
    if file_fields.is_empty() {
        return;
    }

    output.push_str(".PP\n");
    output.push_str("Configuration keys:\n");
    for field in file_fields {
        let file = field.file.as_ref().expect("filtered for file fields");
        output.push_str(".TP\n");
        output.push_str(&bold(&file.key_path));
        output.push('\n');
        output.push_str(&escape_text(&field.help));
        output.push('\n');
    }
}

const fn format_config_format(format: &crate::schema::ConfigFormat) -> &'static str {
    match format {
        crate::schema::ConfigFormat::Toml => "TOML",
        crate::schema::ConfigFormat::Yaml => "YAML",
        crate::schema::ConfigFormat::Json => "JSON",
    }
}

/// Generates the PRECEDENCE section content.
#[expect(
    clippy::format_push_string,
    reason = "roff templating uses format! for clarity"
)]
pub fn precedence_section(
    headings: &LocalizedHeadings,
    precedence: Option<&LocalizedPrecedenceMeta>,
) -> String {
    let prec = match precedence {
        Some(p) if !p.order.is_empty() => p,
        _ => return String::new(),
    };

    let mut output = format!(".SH {}\n", headings.precedence);
    output.push_str(concat!(
        "Configuration values are resolved in the following order ",
        "(highest precedence last):\n"
    ));
    output.push_str(".RS\n");

    for (i, source) in prec.order.iter().enumerate() {
        let num = i + 1;
        let name = format_source_kind(source);
        output.push_str(&format!(".IP {num}. 4\n{name}\n"));
    }

    output.push_str(".RE\n");

    if let Some(rationale) = &prec.rationale {
        output.push_str(".PP\n");
        output.push_str(&escape_text(rationale));
        output.push('\n');
    }

    output
}

const fn format_source_kind(kind: &SourceKind) -> &'static str {
    match kind {
        SourceKind::Defaults => "Built-in defaults",
        SourceKind::File => "Configuration files",
        SourceKind::Env => "Environment variables",
        SourceKind::Cli => "Command-line arguments",
    }
}

/// Generates the EXAMPLES section content.
pub fn examples_section(headings: &LocalizedHeadings, examples: &[LocalizedExample]) -> String {
    if examples.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", headings.examples);

    for example in examples {
        if let Some(title) = &example.title {
            output.push_str(".TP\n");
            output.push_str(&bold(title));
            output.push('\n');
        }

        // Render code in no-fill mode
        output.push_str(".nf\n");
        output.push_str(&escape_text(&example.code));
        output.push_str("\n.fi\n");

        if let Some(body) = &example.body {
            output.push_str(&escape_text(body));
            output.push('\n');
        }
    }

    output
}

/// Generates the SEE ALSO section content.
#[expect(
    clippy::format_push_string,
    reason = "roff templating uses format! for clarity"
)]
pub fn see_also_section(
    headings: &LocalizedHeadings,
    links: &[LocalizedLink],
    related_commands: &[String],
) -> String {
    if links.is_empty() && related_commands.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", headings.see_also);

    // Related commands first
    for cmd in related_commands {
        let escaped_cmd = escape_text(cmd);
        output.push_str(&format!(".BR {escaped_cmd} (1),\n"));
    }

    // Then links
    for link in links {
        if let Some(text) = &link.text {
            output.push_str(&format!(".UR {}\n{}\n.UE ,\n", link.uri, escape_text(text)));
        } else {
            output.push_str(&format!(".UR {}\n.UE ,\n", link.uri));
        }
    }

    output
}

/// Generates the EXIT STATUS section content with standard values.
pub fn exit_status_section(headings: &LocalizedHeadings) -> String {
    // Only generate if the heading is present (non-empty)
    if headings.exit_status.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", headings.exit_status);
    output.push_str(".TP\n");
    output.push_str(&bold("0"));
    output.push_str("\nSuccessful execution.\n");
    output.push_str(".TP\n");
    output.push_str(&bold("1"));
    output.push_str("\nGeneral errors.\n");

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_headings() -> LocalizedHeadings {
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

    #[test]
    fn title_header_formats_correctly() {
        let metadata = TitleMetadata::new(Some("2026-01-31"), Some("v1.0"), Some("User Commands"));
        let result = title_header("my-app", 1, &metadata);
        assert!(result.starts_with(".TH \"MY-APP\" \"1\""));
        assert!(result.contains("2026-01-31"));
        assert!(result.contains("v1.0"));
        assert!(result.contains("User Commands"));
    }

    #[test]
    fn name_section_escapes_description() {
        let headings = test_headings();
        // Only dashes at line start are escaped (to prevent option interpretation)
        // The dash in "A -test" is not at line start, so it stays as-is
        let result = name_section(&headings, "my-app", "A -test application");
        assert!(result.contains("my-app \\- A -test application"));

        // Test with a leading dash which SHOULD be escaped
        let leading_dash_result = name_section(&headings, "my-app", "-starts with dash");
        assert!(leading_dash_result.contains("my-app \\- \\-starts with dash"));
    }

    #[test]
    fn precedence_section_orders_sources() {
        let headings = test_headings();
        let prec = LocalizedPrecedenceMeta {
            order: vec![
                SourceKind::Defaults,
                SourceKind::File,
                SourceKind::Env,
                SourceKind::Cli,
            ],
            rationale: None,
        };
        let result = precedence_section(&headings, Some(&prec));
        assert!(result.contains(".IP 1. 4\nBuilt-in defaults"));
        assert!(result.contains(".IP 4. 4\nCommand-line arguments"));
    }
}
