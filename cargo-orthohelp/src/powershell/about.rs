//! About topic rendering for `PowerShell` help output.

use crate::ir::{
    LocalizedConfigDiscoveryMeta, LocalizedDocMetadata, LocalizedPathPattern,
    LocalizedPrecedenceMeta,
};
use crate::schema::SourceKind;

const CRLF: &str = "\r\n";

/// Renders the about topic content for a module.
#[must_use]
pub fn render_about(metadata: &LocalizedDocMetadata, module_name: &str) -> String {
    let mut output = String::new();

    push_line(&mut output, "TOPIC");
    push_line(&mut output, &format!("    about_{module_name}"));
    push_line(&mut output, "");

    push_line(&mut output, "SHORT DESCRIPTION");
    push_line(&mut output, &format!("    {}", metadata.about));
    push_line(&mut output, "");

    push_line(&mut output, "LONG DESCRIPTION");
    push_line(&mut output, &format!("    {}", metadata.about));

    render_synopsis(&mut output, metadata);
    render_discovery(&mut output, metadata.sections.discovery.as_ref());
    render_precedence(&mut output, metadata.sections.precedence.as_ref());

    output
}

fn render_synopsis(output: &mut String, metadata: &LocalizedDocMetadata) {
    let Some(synopsis) = metadata.synopsis.as_ref() else {
        return;
    };
    push_line(output, "");
    push_line(output, "    Synopsis:");
    push_line(output, &format!("      {synopsis}"));
}

fn render_discovery(output: &mut String, discovery: Option<&LocalizedConfigDiscoveryMeta>) {
    let Some(discovery_meta) = discovery else {
        return;
    };
    push_line(output, "");
    push_line(output, "    Configuration discovery:");
    if discovery_meta.search_paths.is_empty() {
        push_line(output, "      (not specified)");
        return;
    }
    for path in &discovery_meta.search_paths {
        push_line(output, &format_discovery_path(path));
    }
}

fn render_precedence(output: &mut String, precedence: Option<&LocalizedPrecedenceMeta>) {
    let Some(precedence_meta) = precedence else {
        return;
    };
    push_line(output, "");
    push_line(output, "    Precedence:");
    for source in &precedence_meta.order {
        push_line(output, &format!("      - {}", source_label(source)));
    }
    if let Some(rationale) = precedence_meta.rationale.as_ref() {
        push_line(output, "");
        push_line(output, &format!("    {rationale}"));
    }
}

fn format_discovery_path(path: &LocalizedPathPattern) -> String {
    path.note
        .as_deref()
        .filter(|note| !note.is_empty())
        .map_or_else(
            || format!("      - {}", path.pattern),
            |note| format!("      - {} ({})", path.pattern, note),
        )
}

fn push_line(buffer: &mut String, line: &str) {
    buffer.push_str(line);
    buffer.push_str(CRLF);
}

const fn source_label(source: &SourceKind) -> &'static str {
    match source {
        SourceKind::Defaults => "Defaults",
        SourceKind::File => "File",
        SourceKind::Env => "Environment",
        SourceKind::Cli => "CLI",
    }
}
