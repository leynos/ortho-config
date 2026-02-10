//! Shared text normalization helpers for behavioural test suites.

/// Strips one layer of matching single or double quotes from a value.
#[must_use]
pub fn unquote(value: &str) -> &str {
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

/// Normalizes a scalar placeholder by trimming and unquoting one outer layer.
#[must_use]
pub fn normalize_scalar(value: &str) -> String {
    unquote(value).trim().to_owned()
}

/// Removes Unicode bidi isolate markers inserted by some localised renderers.
#[must_use]
pub fn strip_isolates(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(*ch, '\u{2068}' | '\u{2069}'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{normalize_scalar, strip_isolates, unquote};

    #[test]
    fn unquote_removes_single_outer_quotes() {
        assert_eq!(unquote("'value'"), "value");
        assert_eq!(unquote("\"value\""), "value");
    }

    #[test]
    fn normalize_scalar_trims_whitespace() {
        assert_eq!(normalize_scalar("  'value'  "), "value");
    }

    #[test]
    fn strip_isolates_removes_bidi_marks() {
        assert_eq!(strip_isolates("\u{2068}hello\u{2069}"), "hello");
    }
}
