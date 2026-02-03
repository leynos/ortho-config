//! Field entry formatters for roff man page sections.
//!
//! Provides functions to format individual field entries for OPTIONS,
//! ENVIRONMENT, and FILES sections.

use crate::ir::{LocalizedConfigDiscoveryMeta, LocalizedFieldMetadata};
use crate::schema::{CliMetadata, ConfigFormat};

use super::escape::{bold, escape_text, format_flag, format_flag_with_value, italic};

/// Formats a CLI option entry for the OPTIONS section.
pub fn format_option_entry(field: &LocalizedFieldMetadata, cli: &CliMetadata) -> String {
    let mut output = String::new();

    // Tag paragraph with flag
    output.push_str(".TP\n");

    let flag_line = format_option_flag_line(field, cli);
    output.push_str(&flag_line);
    output.push('\n');

    // Help text
    let help = field.long_help.as_deref().unwrap_or(&field.help);
    output.push_str(&escape_text(help));
    output.push('\n');

    // Default value
    if let Some(default) = &field.default {
        output.push_str(".br\nDefault: ");
        output.push_str(&bold(&default.display));
        output.push('\n');
    }

    // Possible values for enums
    if !cli.possible_values.is_empty() {
        output.push_str(".br\nPossible values: ");
        let values = cli.possible_values.join(", ");
        output.push_str(&italic(&values));
        output.push('\n');
    }

    // Deprecation notice
    if let Some(deprecated) = &field.deprecated {
        output.push_str(".br\nDEPRECATED: ");
        output.push_str(&escape_text(&deprecated.note));
        output.push('\n');
    }

    output
}

/// Formats the flag line for an option entry.
fn format_option_flag_line(field: &LocalizedFieldMetadata, cli: &CliMetadata) -> String {
    if cli.takes_value {
        let placeholder = field
            .value
            .as_ref()
            .map(super::escape::value_type_placeholder);
        let value_name = cli
            .value_name
            .as_deref()
            .or(placeholder.as_deref())
            .unwrap_or("VALUE");
        format_flag_with_value(cli.long.as_deref(), cli.short, value_name)
    } else {
        format_flag(cli.long.as_deref(), cli.short)
    }
}

/// Formats an environment variable entry for the ENVIRONMENT section.
pub fn format_env_entry(
    field: &LocalizedFieldMetadata,
    env: &crate::schema::EnvMetadata,
) -> String {
    let mut output = String::new();

    output.push_str(".TP\n");
    output.push_str(&bold(&env.var_name));
    output.push('\n');
    output.push_str(&escape_text(&field.help));

    // Cross-reference CLI flag if available
    if let Some(cli) = &field.cli {
        if let Some(long) = &cli.long {
            output.push_str(" Equivalent to ");
            output.push_str(&bold(&format!("--{long}")));
            output.push('.');
        }
    }
    output.push('\n');

    output
}

/// Formats a file field entry for the FILES section.
pub fn format_file_entry(
    field: &LocalizedFieldMetadata,
    file: &crate::schema::FileMetadata,
) -> String {
    let mut output = String::new();

    output.push_str(".TP\n");
    output.push_str(&bold(&file.key_path));
    output.push('\n');
    output.push_str(&escape_text(&field.help));
    output.push('\n');

    output
}

/// Renders the discovery section content (search paths, formats, XDG compliance).
pub fn render_discovery_section(disc: &LocalizedConfigDiscoveryMeta) -> String {
    let mut output = String::new();

    render_search_paths(&mut output, disc);
    render_supported_formats(&mut output, disc);
    render_xdg_compliance(&mut output, disc);

    output
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

fn render_supported_formats(output: &mut String, disc: &LocalizedConfigDiscoveryMeta) {
    if disc.formats.is_empty() {
        return;
    }

    let formats: Vec<_> = disc.formats.iter().map(format_config_format).collect();
    output.push_str(".PP\nSupported formats: ");
    output.push_str(&formats.join(", "));
    output.push_str(".\n");
}

fn render_xdg_compliance(output: &mut String, disc: &LocalizedConfigDiscoveryMeta) {
    if disc.xdg_compliant {
        output.push_str(".PP\n");
        output.push_str("Configuration discovery follows the XDG Base Directory specification.\n");
    }
}

const fn format_config_format(format: &ConfigFormat) -> &'static str {
    match format {
        ConfigFormat::Toml => "TOML",
        ConfigFormat::Yaml => "YAML",
        ConfigFormat::Json => "JSON",
    }
}
