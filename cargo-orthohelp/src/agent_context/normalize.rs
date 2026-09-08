//! Default-display normalization helpers for agent-context inputs.
//!
//! Split from `mod` so each module stays beneath the 400-line cap.

/// Normalizes displayed Rust paths while preserving all literal contents.
pub(super) fn normalize_default_display(display: &str) -> String {
    let mut normalized = String::with_capacity(display.len());
    let mut chars = display.chars().peekable();
    let mut literal = LiteralState::default();

    while let Some(character) = chars.next() {
        if literal.copy_character(character, &mut normalized, &mut chars) {
            continue;
        }
        if character == '"' {
            literal.start_string(&normalized);
            normalized.push(character);
        } else if character == '\'' && starts_character_literal(&chars) {
            literal.start_character();
            normalized.push(character);
        } else if character == ':' && chars.peek() == Some(&':') {
            normalize_path_separator(&mut normalized, &mut chars);
        } else {
            normalized.push(character);
        }
    }

    normalized
}

#[derive(Default)]
struct LiteralState {
    quote: Option<char>,
    is_escaped: bool,
    raw_hashes: Option<usize>,
}

impl LiteralState {
    /// Enters a quoted or raw string based on its opening prefix.
    fn start_string(&mut self, prefix: &str) {
        self.raw_hashes = raw_string_hash_count(prefix);
        self.quote = self.raw_hashes.is_none().then_some('"');
    }

    /// Enters a character literal at its opening quote.
    const fn start_character(&mut self) {
        self.quote = Some('\'');
    }

    /// Copies a literal character and reports whether it was consumed.
    fn copy_character(
        &mut self,
        character: char,
        normalized: &mut String,
        chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    ) -> bool {
        if let Some(hash_count) = self.raw_hashes {
            normalized.push(character);
            if character == '"' && next_chars_are_hashes(chars, hash_count) {
                copy_hashes(normalized, chars, hash_count);
                self.raw_hashes = None;
            }
            return true;
        }
        let Some(quote) = self.quote else {
            return false;
        };

        normalized.push(character);
        if self.is_escaped {
            self.is_escaped = false;
        } else if character == '\\' {
            self.is_escaped = true;
        } else if character == quote {
            self.quote = None;
        }
        true
    }
}

/// Distinguishes a complete character literal from lifetime syntax.
fn starts_character_literal(chars: &std::iter::Peekable<std::str::Chars<'_>>) -> bool {
    let mut lookahead = chars.clone();
    match lookahead.next() {
        Some('\\') => escaped_character_has_closing_quote(&mut lookahead),
        Some('\'' | '\n' | '\r') | None => false,
        Some(_) => lookahead.next() == Some('\''),
    }
}

/// Checks that an escaped character sequence ends with a closing quote.
fn escaped_character_has_closing_quote(chars: &mut impl Iterator<Item = char>) -> bool {
    match chars.next() {
        Some('x') => chars.nth(2) == Some('\''),
        Some('u') if chars.next() == Some('{') => {
            chars.any(|character| character == '}') && chars.next() == Some('\'')
        }
        Some(_) => chars.next() == Some('\''),
        None => false,
    }
}

/// Returns the delimiter width for a valid raw-string prefix.
fn raw_string_hash_count(prefix: &str) -> Option<usize> {
    let hash_count = prefix.chars().rev().take_while(|char| *char == '#').count();
    let before_hashes = prefix.trim_end_matches('#');
    let before_marker = ["br", "cr", "r"]
        .into_iter()
        .find_map(|marker| before_hashes.strip_suffix(marker))?;

    (!before_marker
        .chars()
        .next_back()
        .is_some_and(|char| char == '_' || char.is_alphanumeric()))
    .then_some(hash_count)
}

/// Checks whether the next `hash_count` characters close a raw string.
fn next_chars_are_hashes(
    chars: &std::iter::Peekable<std::str::Chars<'_>>,
    hash_count: usize,
) -> bool {
    chars.clone().take(hash_count).all(|char| char == '#')
}

/// Copies a raw string's closing hashes into the normalized output.
fn copy_hashes(
    normalized: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    hash_count: usize,
) {
    for _ in 0..hash_count {
        if let Some(hash) = chars.next() {
            normalized.push(hash);
        }
    }
}

/// Replaces a path separator and its surrounding whitespace with `::`.
fn normalize_path_separator(
    normalized: &mut String,
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
) {
    while normalized
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace)
    {
        normalized.pop();
    }
    normalized.push_str("::");
    chars.next();
    while chars.peek().is_some_and(|next| next.is_whitespace()) {
        chars.next();
    }
}
