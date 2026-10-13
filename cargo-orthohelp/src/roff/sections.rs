//! Section content generators for man pages.
//!
//! Provides functions to generate each standard man page section from
//! localized documentation metadata.

use crate::ir::{
    LocalizedConfigDiscoveryMeta, LocalizedExample, LocalizedFieldMetadata, LocalizedHeadings,
    LocalizedLink, LocalizedPrecedenceMeta,
};
use crate::schema::{FileMetadata, SourceKind};

use super::entry;
use super::escape::{bold, escape_macro_arg, escape_text, format_flag, format_flag_with_value};
use super::types::ManSection;

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
pub fn title_header(name: &str, section: ManSection, metadata: &TitleMetadata) -> String {
    let name_upper = escape_macro_arg(&name.to_uppercase());
    let date_str = metadata.date.map_or_else(String::new, escape_macro_arg);
    let source_str = metadata.source.map_or_else(String::new, escape_macro_arg);
    let manual_str = metadata.manual.map_or_else(String::new, escape_macro_arg);
    format!(".TH \"{name_upper}\" \"{section}\" \"{date_str}\" \"{source_str}\" \"{manual_str}\"\n")
}

/// Generates the NAME section content.
pub fn name_section(headings: &LocalizedHeadings, name: &str, about: &str) -> String {
    let escaped_about = escape_text(about);
    format!(
        ".SH {}\n{name} \\- {escaped_about}\n",
        escape_macro_arg(&headings.name)
    )
}

/// Generates the SYNOPSIS section content.
pub fn synopsis_section(
    headings: &LocalizedHeadings,
    bin_name: &str,
    synopsis: Option<&str>,
    fields: &[LocalizedFieldMetadata],
) -> String {
    let mut output = format!(
        ".SH {}\n.B {bin_name}\n",
        escape_macro_arg(&headings.synopsis)
    );

    if let Some(syn) = synopsis {
        output.push_str(&escape_text(syn));
        output.push('\n');
    } else {
        let visible_cli_fields = fields
            .iter()
            .filter_map(|f| f.cli.as_ref().filter(|c| !c.hide_in_help).map(|c| (f, c)));
        for (field, cli) in visible_cli_fields {
            output.push_str(&format_synopsis_option(field, cli));
        }
    }

    output
}

fn format_synopsis_option(
    field: &LocalizedFieldMetadata,
    cli: &crate::schema::CliMetadata,
) -> String {
    let flag = if cli.takes_value {
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
    format!(
        ".SH {}\n{escaped}\n",
        escape_macro_arg(&headings.description)
    )
}

/// Generates the OPTIONS section content.
pub fn options_section(headings: &LocalizedHeadings, fields: &[LocalizedFieldMetadata]) -> String {
    let cli_fields: Vec<_> = fields
        .iter()
        .filter_map(|f| f.cli.as_ref().filter(|c| !c.hide_in_help).map(|c| (f, c)))
        .collect();

    if cli_fields.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", escape_macro_arg(&headings.options));
    for (field, cli) in cli_fields {
        output.push_str(&entry::format_option_entry(field, cli));
    }
    output
}

/// Generates the ENVIRONMENT section content.
pub fn environment_section(
    headings: &LocalizedHeadings,
    fields: &[LocalizedFieldMetadata],
) -> String {
    let mut env_fields: Vec<_> = fields
        .iter()
        .filter_map(|f| f.env.as_ref().map(|e| (f, e)))
        .collect();

    if env_fields.is_empty() {
        return String::new();
    }

    env_fields.sort_by(|(_, a), (_, b)| a.var_name.cmp(&b.var_name));

    let mut output = format!(".SH {}\n", escape_macro_arg(&headings.environment));
    for (field, env) in env_fields {
        output.push_str(&entry::format_env_entry(field, env));
    }
    output
}

/// Generates the FILES section content.
pub fn files_section(
    headings: &LocalizedHeadings,
    fields: &[LocalizedFieldMetadata],
    discovery: Option<&LocalizedConfigDiscoveryMeta>,
) -> String {
    let file_fields: Vec<_> = fields
        .iter()
        .filter_map(|f| f.file.as_ref().map(|file| (f, file)))
        .collect();
    let has_discovery = discovery.is_some_and(has_discovery_content);

    if file_fields.is_empty() && !has_discovery {
        return String::new();
    }

    let mut output = format!(".SH {}\n", escape_macro_arg(&headings.files));

    // Discovery section
    if let Some(disc) = discovery.filter(|d| has_discovery_content(d)) {
        output.push_str(&entry::render_discovery_section(disc));
    }

    // File fields section
    render_file_keys(&mut output, &file_fields);

    output
}

fn render_file_keys(output: &mut String, file_fields: &[(&LocalizedFieldMetadata, &FileMetadata)]) {
    if file_fields.is_empty() {
        return;
    }

    output.push_str(".PP\nConfiguration keys:\n");
    for (field, file) in file_fields {
        output.push_str(&entry::format_file_entry(field, file));
    }
}

/// Checks whether discovery metadata has any renderable content.
const fn has_discovery_content(d: &LocalizedConfigDiscoveryMeta) -> bool {
    !d.search_paths.is_empty() || !d.formats.is_empty() || d.xdg_compliant
}

/// Generates the PRECEDENCE section content.
pub fn precedence_section(
    headings: &LocalizedHeadings,
    precedence: Option<&LocalizedPrecedenceMeta>,
) -> String {
    let prec = match precedence {
        Some(p) if !p.order.is_empty() => p,
        _ => return String::new(),
    };

    let mut output = format!(".SH {}\n", escape_macro_arg(&headings.precedence));
    output.push_str(concat!(
        "Configuration values are resolved in the following order ",
        "(highest precedence last):\n"
    ));
    output.push_str(".RS\n");

    for (i, source) in prec.order.iter().enumerate() {
        let num = i + 1;
        let name = format_source_kind(source);
        output.push_str(".IP ");
        output.push_str(&num.to_string());
        output.push_str(". 4\n");
        output.push_str(name);
        output.push('\n');
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
        SourceKind::Profile => "Profile overlays",
        SourceKind::Env => "Environment variables",
        SourceKind::Cli => "Command-line arguments",
    }
}

/// Generates the EXAMPLES section content.
pub fn examples_section(headings: &LocalizedHeadings, examples: &[LocalizedExample]) -> String {
    if examples.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", escape_macro_arg(&headings.examples));

    for example in examples {
        if let Some(title) = &example.title {
            output.push_str(".TP\n");
            output.push_str(&bold(title));
            output.push('\n');
        }

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
pub fn see_also_section(
    headings: &LocalizedHeadings,
    links: &[LocalizedLink],
    related_commands: &[String],
    section: ManSection,
) -> String {
    if links.is_empty() && related_commands.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", escape_macro_arg(&headings.see_also));

    for cmd in related_commands {
        let escaped_cmd = escape_macro_arg(cmd);
        output.push_str(".BR ");
        output.push_str(&escaped_cmd);
        output.push_str(" (");
        output.push_str(&section.to_string());
        output.push_str("),\n");
    }

    for link in links {
        output.push_str(".UR ");
        output.push_str(&escape_macro_arg(&link.uri));
        output.push('\n');
        if let Some(text) = &link.text {
            output.push_str(&escape_text(text));
            output.push('\n');
        }
        output.push_str(".UE ,\n");
    }

    output
}

/// Generates the EXIT STATUS section content with standard values.
pub fn exit_status_section(headings: &LocalizedHeadings) -> String {
    if headings.exit_status.is_empty() {
        return String::new();
    }

    let mut output = format!(".SH {}\n", escape_macro_arg(&headings.exit_status));
    output.push_str(".TP\n");
    output.push_str(&bold("0"));
    output.push_str("\nSuccessful execution.\n");
    output.push_str(".TP\n");
    output.push_str(&bold("1"));
    output.push_str("\nGeneral errors.\n");

    output
}

#[cfg(test)]
#[path = "sections_tests.rs"]
mod tests;
