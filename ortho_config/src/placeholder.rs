//! Utilities for placeholder pattern validation.
//!
//! Provides `compile_placeholder` which validates brace usage before
//! compiling the pattern into a [`Regex`].

use regex::Regex;

use crate::OrthoError;

fn validate_braces(pattern: &str) -> Result<(), String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut depth = 0;
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        match c {
            '\\' => {
                // Skip the escaped character to avoid interpreting it.
                i += 2;
            }
            '{' => {
                if chars.get(i + 1) == Some(&'{') {
                    // Double braces represent a literal '{'.
                    i += 2;
                } else {
                    depth += 1;
                    i += 1;
                }
            }
            '}' => {
                if chars.get(i + 1) == Some(&'}') {
                    // Literal closing brace.
                    i += 2;
                } else {
                    if depth == 0 {
                        return Err(format!("unmatched '}}' at position {i}"));
                    }
                    depth -= 1;
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    if depth != 0 {
        return Err("unclosed '{' in pattern".to_string());
    }
    Ok(())
}

fn unescape_double_braces(pattern: &str) -> String {
    let mut out = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                out.push_str(r"\{");
                chars.next();
            }
            '}' if chars.peek() == Some(&'}') => {
                out.push_str(r"\}");
                chars.next();
            }
            _ => out.push(c),
        }
    }
    out
}

/// Validate a placeholder pattern and compile it into a [`Regex`].
///
/// The validator understands escaped braces (e.g. `\\{`) and double braces
/// (e.g. `{{`), allowing patterns that would otherwise be rejected as
/// malformed.
///
/// # Errors
///
/// Returns [`OrthoError::PlaceholderSyntax`] when the braces are
/// mismatched and [`OrthoError::PlaceholderRegex`] if regex compilation fails.
#[expect(
    clippy::result_large_err,
    reason = "Return OrthoError to keep a single error type across the public API"
)]
pub fn compile_placeholder(pattern: &str) -> Result<Regex, OrthoError> {
    validate_braces(pattern).map_err(|m| OrthoError::PlaceholderSyntax {
        pattern: pattern.to_string(),
        message: m,
    })?;
    let processed = unescape_double_braces(pattern);
    Regex::new(&processed).map_err(|e| OrthoError::PlaceholderRegex {
        pattern: pattern.to_string(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::compile_placeholder;
    use crate::OrthoError;

    #[test]
    fn accepts_escaped_and_nested_braces() {
        compile_placeholder(r"foo\{bar\}").expect("escaped braces");
        compile_placeholder("{{name}}=").expect("double braces");
    }

    #[test]
    fn rejects_unbalanced_braces() {
        let err = compile_placeholder("{foo").expect_err("invalid");
        assert!(matches!(err, OrthoError::PlaceholderSyntax { .. }));
    }
}
