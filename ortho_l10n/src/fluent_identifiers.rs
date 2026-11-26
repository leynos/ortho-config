//! Identifier normalization shared by catalogue loading and runtime lookup.
//!
//! Catalogues and callers may spell message identifiers with dots, while
//! `Fluent` requires hyphen-separated identifiers. These helpers rewrite only
//! top-level assignment keys, leaving message bodies, terms, and attributes
//! untouched, and normalize runtime lookups without allocating when no rewrite
//! is needed. Keeping them separate leaves the bundle-building module focused
//! on construction and well inside the repository's module-size limit.

use crate::MESSAGE_ID_SEPARATOR;
use std::borrow::Cow;

/// Converts runtime dotted message paths to `Fluent`'s hyphen-separated identifier spelling.
///
/// IDs without dots are borrowed unchanged, avoiding allocation on the common lookup path.
pub(crate) fn normalize_identifier(id: &str) -> Cow<'_, str> {
    if id.contains('.') {
        Cow::Owned(id.replace('.', MESSAGE_ID_SEPARATOR))
    } else {
        Cow::Borrowed(id)
    }
}

/// Normalizes top-level catalogue IDs while retaining message bodies and line boundaries.
pub(crate) fn normalize_resource_ids(resource: &str) -> String {
    resource
        .lines()
        .map(normalize_id_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns true when `ch` is accepted by the load-time id normalizer.
///
/// Fluent identifiers are ASCII-only and do not permit `.`. This load-time
/// pre-normalization predicate is intentionally more tolerant so dotted
/// catalogue ids and existing non-ASCII resource ids can be normalized or
/// passed through before Fluent parses them.
fn is_valid_load_time_id_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

/// Validates a load-time resource identifier before pre-normalization.
fn is_valid_load_time_identifier(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_alphabetic() && chars.all(is_valid_load_time_id_char)
}

/// Rewrites only a valid, unindented assignment key; comments and Fluent body lines pass through.
fn normalize_id_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return line.to_owned();
    }

    let Some((left, right)) = line.split_once('=') else {
        return line.to_owned();
    };

    // Only normalize top-level identifiers. Indented lines belong to message
    // bodies, attributes, or variants and must be left untouched.
    if left.chars().next().is_some_and(char::is_whitespace) {
        return line.to_owned();
    }

    let id_segment = left.trim_end();
    if !is_valid_load_time_identifier(id_segment) {
        return line.to_owned();
    }

    let normalized_id = normalize_identifier(id_segment).into_owned();
    let trailing_ws = left.strip_prefix(id_segment).unwrap_or_default();

    let mut rebuilt = String::with_capacity(line.len());
    rebuilt.push_str(&normalized_id);
    rebuilt.push_str(trailing_ws);
    rebuilt.push('=');
    rebuilt.push_str(right);
    rebuilt
}

#[cfg(test)]
mod tests {
    //! Tests for Fluent identifier normalization.

    use super::*;

    #[test]
    fn normalizes_top_level_dotted_ids() {
        let resource = "cli.about = About text";
        let normalized = normalize_resource_ids(resource);
        assert_eq!(
            normalized, "cli-about = About text",
            "a top-level dotted id should be normalized to hyphen spelling: {normalized:?}"
        );
    }

    #[test]
    fn leaves_indented_lines_with_equals_untouched() {
        let resource = "message =\n    section.title = Title line";
        let normalized = normalize_resource_ids(resource);
        assert!(
            normalized.contains("    section.title = Title line"),
            "an indented assignment line should pass through unchanged: {normalized:?}"
        );
    }

    #[test]
    fn leaves_terms_and_attributes_unchanged() {
        let resource = "-term.id = Term value\nmessage = Value\n    .label = Label";
        let normalized = normalize_resource_ids(resource);
        assert!(
            normalized.contains("-term.id = Term value"),
            "a term definition should pass through unchanged: {normalized:?}"
        );
        assert!(
            normalized.contains("    .label = Label"),
            "an attribute line should pass through unchanged: {normalized:?}"
        );
    }

    #[test]
    fn normalizes_unicode_identifiers() {
        let resource = "ключ.значение = Привет";
        let normalized = normalize_resource_ids(resource);
        assert!(
            normalized.starts_with("ключ-значение = Привет"),
            "a non-ASCII dotted id should normalize while its value survives: {normalized:?}"
        );
    }
}
