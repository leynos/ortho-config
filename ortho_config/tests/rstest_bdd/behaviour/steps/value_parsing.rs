//! Shared value parsing helpers for BDD step placeholders.

use ortho_config::OrthoError;
use test_helpers::text;

/// Strips one layer of matching single or double quotes from a value.
pub(crate) fn unquote(value: &str) -> &str { text::unquote(value) }

/// Normalises a scalar placeholder value by trimming and unquoting it.
pub(crate) fn normalize_scalar(value: &str) -> String { text::normalize_scalar(value) }

/// Parses a comma-separated placeholder value while tolerating outer quotes.
pub(crate) fn parse_csv_values(value: &str) -> Vec<String> {
    unquote(value)
        .split(',')
        .map(normalize_scalar)
        .filter(|item| !item.is_empty())
        .collect()
}

/// Removes Unicode bidi isolate markers inserted by some localised renderers.
pub(crate) fn strip_isolates(value: &str) -> String { text::strip_isolates(value) }

/// Returns true when the error is a CLI parsing error, directly or aggregated.
pub(crate) fn is_cli_parsing_error(err: &OrthoError) -> bool {
    match err {
        OrthoError::CliParsing(_) => true,
        OrthoError::Aggregate(agg) => agg
            .iter()
            .any(|entry| matches!(entry, OrthoError::CliParsing(_))),
        _ => false,
    }
}
