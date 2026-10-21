//! Property tests for the rustflags sanitizer.
//!
//! These are sampled checks, not proofs. Each case generates a token sequence
//! for one delimiter, computes the surviving tokens with an independent
//! reference filter, and compares the exact output text. Comparing the whole
//! string rather than just the token set is what makes order preservation,
//! delimiter preservation, and the byte-identical passthrough observable: a
//! sanitizer that reordered tokens or re-joined them with the wrong separator
//! would still satisfy a set-based expectation.
//!
//! The reference filter is written from the documented contract rather than
//! derived from the implementation: a token is removed when it is a compact
//! coverage flag, or when it is a bare `-C` whose successor names the coverage
//! option. Its "was anything removed" verdict drives the expected value, so
//! `None` is expected exactly when every token is removed — the behaviour the
//! caller depends on, since `None` tells it to remove the environment variable
//! rather than set it to an empty string.
//!
//! The generators are shaped so the properties are non-vacuous. Whitespace
//! cases never generate a token containing whitespace or an empty token, and
//! encoded cases never generate the unit separator, so a generated value can
//! only be ambiguous if the sanitizer itself were faulty. The general
//! sequences always end with a coverage token, and a separate generator
//! produces sequences made entirely of removable units, so both the partial
//! removal and the remove-everything branches are reached rather than skipped.

use proptest::prelude::*;

use super::{Delimiter, ENCODED_RUSTFLAGS_SEPARATOR, RustflagsValue, sanitize};

/// One generated rustflags token before interpretation.
#[derive(Clone, Copy, Debug)]
enum Token {
    /// An ordinary user flag the sanitizer must keep.
    User(&'static str),
    /// A bare `-C`, which only names coverage when the next token does.
    SplitCodegen,
    /// The option name that follows a split `-C`.
    CoverageOption(&'static str),
    /// A compact `-C`-prefixed coverage flag.
    CompactCoverage(&'static str),
}

/// User flags that must always survive sanitization.
const USER_TOKENS: &[&str] = &[
    "--cfg",
    "caller_gate",
    "-Clink-arg=/SAFESEH:NO",
    "-Copt-level=3",
    "--deny",
    "warnings",
];

/// The split-form coverage option spellings the sanitizer strips.
const COVERAGE_OPTIONS: &[&str] = &["instrument-coverage", "instrument-coverage=token"];

/// The compact-form coverage flag spellings the sanitizer strips.
const COMPACT_COVERAGE: &[&str] = &["-Cinstrument-coverage", "-Cinstrument-coverage=token"];

/// The text a value's tokens are joined with for `delimiter`.
fn separator(delimiter: Delimiter) -> String {
    match delimiter {
        Delimiter::Whitespace => " ".to_owned(),
        Delimiter::UnitSeparator => ENCODED_RUSTFLAGS_SEPARATOR.to_string(),
    }
}

/// Renders one token as the text the sanitizer will split back out.
fn token_text(token: Token) -> &'static str {
    match token {
        Token::User(text) | Token::CoverageOption(text) | Token::CompactCoverage(text) => text,
        Token::SplitCodegen => "-C",
    }
}

/// Renders a whole token sequence as the input value for `delimiter`.
fn render(tokens: &[Token], delimiter: Delimiter) -> String {
    tokens
        .iter()
        .map(|token| token_text(*token))
        .collect::<Vec<_>>()
        .join(&separator(delimiter))
}

/// True when the token names the coverage option after a split `-C`.
fn is_coverage_option(token: Token) -> bool {
    matches!(token, Token::CoverageOption(_))
}

/// True when the token is a compact coverage flag.
const fn is_compact_coverage(token: Token) -> bool {
    matches!(token, Token::CompactCoverage(_))
}

/// True when `tokens[index]` begins a two-token split coverage flag.
///
/// The guard is written against the successor so the caller never needs to
/// know whether the pair fits inside the sequence.
fn split_coverage_at(tokens: &[Token], index: usize) -> bool {
    matches!(tokens.get(index), Some(Token::SplitCodegen))
        && tokens
            .get(index + 1)
            .copied()
            .is_some_and(is_coverage_option)
}

/// Applies the documented sanitization rule to one token sequence.
///
/// Returns the tokens the reference filter keeps. A `-C` becomes a pair with
/// its successor only when that successor is a coverage option; otherwise it
/// is an ordinary user flag and survives on its own.
///
/// The retained tokens are copied out rather than the surviving indices, so
/// callers never index the input with a value derived from its own length;
/// `tokens.get` already bounds the walk.
fn reference_retained(tokens: &[Token]) -> Vec<Token> {
    let mut retained = Vec::with_capacity(tokens.len());
    let mut index = 0;
    while let Some(token) = tokens.get(index).copied() {
        if is_compact_coverage(token) {
            index += 1;
            continue;
        }
        if split_coverage_at(tokens, index) {
            index += 2;
            continue;
        }
        retained.push(token);
        index += 1;
    }
    retained
}

/// The exact output the sanitizer must produce for `tokens` and `delimiter`.
///
/// A sequence with nothing removed passes through byte for byte, so the
/// expected value is the input verbatim rather than a re-joined copy. That
/// distinction matters for whitespace values carrying runs of spaces, which
/// re-joining would collapse.
fn reference_output(tokens: &[Token], delimiter: Delimiter) -> Option<String> {
    let retained = reference_retained(tokens);
    if retained.len() == tokens.len() {
        return Some(render(tokens, delimiter));
    }
    if retained.is_empty() {
        return None;
    }
    let survivors = retained.iter().copied().map(token_text);
    Some(survivors.collect::<Vec<_>>().join(&separator(delimiter)))
}

/// A strategy for one token that is legal in both delimiters.
fn any_token() -> impl Strategy<Value = Token> {
    prop_oneof![
        proptest::sample::select(USER_TOKENS).prop_map(Token::User),
        Just(Token::SplitCodegen),
        proptest::sample::select(COVERAGE_OPTIONS).prop_map(Token::CoverageOption),
        proptest::sample::select(COMPACT_COVERAGE).prop_map(Token::CompactCoverage),
    ]
}

/// A strategy for a user flag only.
fn user_token() -> impl Strategy<Value = Token> {
    proptest::sample::select(USER_TOKENS).prop_map(Token::User)
}

/// A bounded token sequence that always contains at least one coverage token.
///
/// The trailing coverage token guarantees every generated case exercises a
/// removal, so the property cannot pass by never reaching the branch it
/// claims to test.
fn token_sequence() -> impl Strategy<Value = Vec<Token>> {
    proptest::collection::vec(any_token(), 0..12).prop_map(|mut tokens| {
        tokens.push(Token::CompactCoverage("-Cinstrument-coverage"));
        tokens
    })
}

/// A strategy for a sequence made entirely of removable units.
///
/// Each unit is either a compact flag or a split `-C` pair, so every token in
/// the sequence is removed and the sanitizer must report `None`.
fn all_coverage_sequence() -> impl Strategy<Value = Vec<Token>> {
    let unit = prop_oneof![
        proptest::sample::select(COMPACT_COVERAGE)
            .prop_map(|text| vec![Token::CompactCoverage(text)]),
        proptest::sample::select(COVERAGE_OPTIONS)
            .prop_map(|text| vec![Token::SplitCodegen, Token::CoverageOption(text),]),
    ];
    proptest::collection::vec(unit, 1..8).prop_map(|units| units.concat())
}

proptest! {
    /// The sanitizer matches the reference filter for whitespace-delimited
    /// values: same survivors, same order, same joining, same verdict.
    #[test]
    fn whitespace_sanitization_matches_the_reference_filter(tokens in token_sequence()) {
        let input = render(&tokens, Delimiter::Whitespace);
        let expected = reference_output(&tokens, Delimiter::Whitespace);
        prop_assert_eq!(sanitize(RustflagsValue(&input), Delimiter::Whitespace), expected);
    }

    /// The same invariant holds for unit-separator-delimited values.
    #[test]
    fn encoded_sanitization_matches_the_reference_filter(tokens in token_sequence()) {
        let input = render(&tokens, Delimiter::UnitSeparator);
        let expected = reference_output(&tokens, Delimiter::UnitSeparator);
        prop_assert_eq!(sanitize(RustflagsValue(&input), Delimiter::UnitSeparator), expected);
    }

    /// Surviving tokens appear exactly once each, in their original order.
    ///
    /// This states the ordering claim the string equality only implies, so a
    /// failure names the ordering defect directly rather than a diff.
    #[test]
    fn surviving_tokens_keep_their_order(tokens in token_sequence()) {
        let retained = reference_retained(&tokens);
        let expected = retained.iter().copied().map(token_text).collect::<Vec<_>>();
        let input = render(&tokens, Delimiter::Whitespace);
        let actual = sanitize(RustflagsValue(&input), Delimiter::Whitespace)
            .map(|sanitized| {
                sanitized
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        prop_assert_eq!(actual, expected);
    }

    /// A sequence whose every token is coverage instrumentation yields `None`.
    ///
    /// `None` is the signal to remove the variable entirely, so returning
    /// `Some("")` here would leave the child process with an empty flag set
    /// that Cargo treats differently from an unset variable.
    #[test]
    fn a_fully_removed_value_yields_none(tokens in all_coverage_sequence()) {
        for delimiter in [Delimiter::Whitespace, Delimiter::UnitSeparator] {
            let input = render(&tokens, delimiter);
            prop_assert_eq!(sanitize(RustflagsValue(&input), delimiter), None);
        }
    }

    /// A value with no coverage token is passed through byte for byte.
    ///
    /// The input is joined with padded runs of spaces, which re-joining would
    /// collapse, so this pins the passthrough rather than a coincidentally
    /// equal re-render.
    #[test]
    fn a_value_without_coverage_flags_is_unchanged(tokens in proptest::collection::vec(user_token(), 0..8)) {
        let input = tokens
            .iter()
            .map(|token| token_text(*token))
            .collect::<Vec<_>>()
            .join("   ");
        prop_assert_eq!(
            sanitize(RustflagsValue(&input), Delimiter::Whitespace),
            Some(input)
        );
    }
}
