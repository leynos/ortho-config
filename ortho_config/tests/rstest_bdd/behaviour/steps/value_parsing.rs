//! Shared value parsing helpers for BDD step placeholders.

/// Strips one layer of matching single or double quotes from a value.
pub(crate) fn unquote(value: &str) -> &str {
    let trimmed = value.trim();
    if let Some(stripped) = trimmed.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        return stripped;
    }
    if let Some(stripped) = trimmed
        .strip_prefix('\'')
        .and_then(|v| v.strip_suffix('\''))
    {
        return stripped;
    }
    trimmed
}

/// Normalises a scalar placeholder value by trimming and unquoting it.
pub(crate) fn normalize_scalar(value: &str) -> String {
    unquote(value).trim().to_owned()
}

/// Parses a comma-separated placeholder value while tolerating outer quotes.
pub(crate) fn parse_csv_values(value: &str) -> Vec<String> {
    unquote(value)
        .split(',')
        .map(normalize_scalar)
        .filter(|item| !item.is_empty())
        .collect()
}

/// Removes Unicode bidi isolate markers inserted by some localised renderers.
pub(crate) fn strip_isolates(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(*ch, '\u{2068}' | '\u{2069}'))
        .collect()
}
