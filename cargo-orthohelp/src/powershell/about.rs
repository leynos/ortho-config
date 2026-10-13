//! About topic rendering for `PowerShell` help output.

use crate::ir::{
    LocalizedConfigDiscoveryMeta, LocalizedDocMetadata, LocalizedPathPattern,
    LocalizedPrecedenceMeta,
};
use crate::powershell::text::push_line;
use crate::schema::SourceKind;

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

const fn source_label(source: &SourceKind) -> &'static str {
    match source {
        SourceKind::Defaults => "Defaults",
        SourceKind::File => "File",
        SourceKind::Profile => "Profile",
        SourceKind::Env => "Environment",
        SourceKind::Cli => "CLI",
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for `PowerShell` about-topic rendering.

    use super::*;
    use crate::powershell::test_fixtures;

    /// The about topic is the only consumer of `source_label`, so an
    /// unasserted arm would let a wrong label ship silently. A
    /// profile-opted-in command lists the profile overlay immediately between
    /// configuration files and the environment.
    #[test]
    fn precedence_block_places_profiles_between_files_and_environment() {
        let mut metadata = test_fixtures::minimal_doc("en-US", "Fixture app");
        metadata.sections.precedence = Some(LocalizedPrecedenceMeta {
            order: vec![
                SourceKind::Defaults,
                SourceKind::File,
                SourceKind::Profile,
                SourceKind::Env,
                SourceKind::Cli,
            ],
            rationale: None,
        });

        let rendered = render_about(&metadata, "Fixture");
        let lines = rendered.lines().collect::<Vec<_>>();
        let index_of = |label: &str| {
            lines
                .iter()
                .position(|line| line.strip_prefix("      - ") == Some(label))
                .unwrap_or_else(|| panic!("missing '{label}' row in:\n{rendered}"))
        };

        let expected = ["Defaults", "File", "Profile", "Environment", "CLI"];
        for (label, next_label) in expected.iter().zip(expected.iter().skip(1)) {
            assert_eq!(
                index_of(next_label),
                index_of(label) + 1,
                "'{next_label}' must immediately follow '{label}' in:\n{rendered}",
            );
        }
    }
}
