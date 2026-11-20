//! Fluent-ID lookup policy for localized documentation.
//!
//! Localization copies structural metadata verbatim and resolves only text.
//! The policy here decides what happens when a required identifier has no
//! translation: the marker keeps the gap visible in the published artefact
//! instead of silently emitting an empty string, and the standard headings fall
//! back to their conventional English labels so a missing bundle degrades to a
//! readable man page rather than a page of markers.

use ortho_config::Localizer;

/// Returns a translation or an explicit marker so unresolved required copy stays visible.
pub(super) fn resolve_message(localizer: &dyn Localizer, id: &str) -> String {
    localizer
        .lookup(id, None)
        .unwrap_or_else(|| format!("[missing: {id}]"))
}

/// Uses a translation, then a built-in standard heading, and marks other missing IDs.
pub(super) fn resolve_heading(localizer: &dyn Localizer, id: &str) -> String {
    localizer.lookup(id, None).unwrap_or_else(|| {
        standard_heading_fallback(id).map_or_else(|| format!("[missing: {id}]"), str::to_owned)
    })
}

/// Supplies English labels only for the recognized standard heading IDs.
fn standard_heading_fallback(id: &str) -> Option<&'static str> {
    match id {
        "ortho.headings.name" => Some("NAME"),
        "ortho.headings.synopsis" => Some("SYNOPSIS"),
        "ortho.headings.description" => Some("DESCRIPTION"),
        "ortho.headings.options" => Some("OPTIONS"),
        "ortho.headings.environment" => Some("ENVIRONMENT"),
        "ortho.headings.files" => Some("FILES"),
        "ortho.headings.precedence" => Some("PRECEDENCE"),
        "ortho.headings.exit_status" => Some("EXIT STATUS"),
        "ortho.headings.examples" => Some("EXAMPLES"),
        "ortho.headings.see_also" => Some("SEE ALSO"),
        "ortho.headings.commands" => Some("COMMANDS"),
        _ => None,
    }
}

/// Keeps absent optional IDs absent and resolves present IDs using the required-message policy.
pub(super) fn resolve_optional(localizer: &dyn Localizer, id: Option<&str>) -> Option<String> {
    id.map(|value| resolve_message(localizer, value))
}
