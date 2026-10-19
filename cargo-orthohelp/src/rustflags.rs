//! Rust flag sanitization for ephemeral bridge builds.
//!
//! When `cargo-orthohelp` spawns `cargo build` to compile the bridge crate, the
//! child process inherits the parent's environment. Under `cargo-llvm-cov` this
//! includes `-Cinstrument-coverage` tokens in `RUSTFLAGS` and
//! `CARGO_ENCODED_RUSTFLAGS`, which cause the bridge binary to embed coverage
//! instrumentation and write profiling data that can interfere with the test run.
//!
//! This module provides [`apply_sanitized_rustflags`], which reads those
//! variables from the current environment, strips any coverage-related tokens,
//! and re-applies the cleaned values to the provided [`Command`]. Variables
//! that become empty after sanitization are removed entirely. It is called
//! exclusively from [`crate::bridge::build_bridge_command`].

use std::process::Command;

/// Cargo's unit-separator delimiter between arguments in `CARGO_ENCODED_RUSTFLAGS`.
const ENCODED_RUSTFLAGS_SEPARATOR: char = '\x1f';

/// A Cargo compiler-flag environment variable this module reads.
///
/// The set is closed and its members differ in more than name: one is
/// whitespace-delimited and one is unit-separator-delimited, so a caller that
/// passed a bare `&str` could pair a variable with the wrong sanitizer. The
/// type makes that mistake unrepresentable.
#[derive(Clone, Copy, Debug)]
struct RustflagsVariable(&'static str);

impl RustflagsVariable {
    /// `RUSTFLAGS`, whitespace-delimited.
    const RUSTFLAGS: Self = Self("RUSTFLAGS");
    /// `CARGO_ENCODED_RUSTFLAGS`, unit-separator-delimited.
    const ENCODED: Self = Self("CARGO_ENCODED_RUSTFLAGS");

    /// The variable name as the operating system spells it.
    const fn as_str(self) -> &'static str {
        self.0
    }
}

/// The raw value of one rustflags environment variable.
#[derive(Clone, Copy, Debug)]
struct RustflagsValue<'a>(&'a str);

impl<'a> RustflagsValue<'a> {
    /// The value text.
    const fn as_str(self) -> &'a str {
        self.0
    }
}

/// One token of a rustflags value, before coverage filtering.
#[derive(Clone, Copy, Debug)]
struct RustflagToken<'a>(&'a str);

impl<'a> RustflagToken<'a> {
    /// The token text.
    const fn as_str(self) -> &'a str {
        self.0
    }
}

/// Applies sanitized Rust compiler flags to `command`.
///
/// Reads `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` from the current process
/// environment, strips any `-Cinstrument-coverage` tokens injected by
/// `cargo-llvm-cov`, and re-applies the sanitized values to `command`.
/// Variables that become empty after sanitization are removed entirely so the
/// child process inherits a clean flag set rather than an empty string.
pub(crate) fn apply_sanitized_rustflags(command: &mut Command) {
    apply_sanitized_rustflags_var(command, RustflagsVariable::RUSTFLAGS, sanitize_plain);
    apply_sanitized_rustflags_var(command, RustflagsVariable::ENCODED, sanitize_encoded);
}

/// Reads the named environment variable, sanitizes it with `sanitize`, and
/// either sets the sanitized value on `command` or removes the variable when
/// sanitization produces no tokens.
fn apply_sanitized_rustflags_var(
    command: &mut Command,
    var: RustflagsVariable,
    sanitize: fn(RustflagsValue<'_>) -> Option<String>,
) {
    let name = var.as_str();
    let Ok(value) = std::env::var(name) else {
        return;
    };
    match sanitize(RustflagsValue(&value)) {
        Some(sanitized) if sanitized == value => {
            // No coverage tokens found; pass through unchanged.
            command.env(name, sanitized);
        }
        Some(sanitized) => {
            log_rustflags_stripped(var);
            command.env(name, sanitized);
        }
        None => {
            log_rustflags_removed(var);
            command.env_remove(name);
        }
    }
}

/// Report that coverage flags were stripped from a rustflags variable.
///
/// Each emission lives in its own function so [`apply_sanitized_rustflags_var`]
/// holds at most one `tracing` call: when a dependent crate enables
/// `tracing`'s `log` bridge, Cargo's feature unification expands every event
/// into a branch ladder that counts against the workspace's
/// cognitive-complexity ceiling.
fn log_rustflags_stripped(var: RustflagsVariable) {
    tracing::debug!(
        var = var.as_str(),
        "stripped coverage flags from rustflags variable"
    );
}

/// Report that a rustflags variable was removed after sanitization.
fn log_rustflags_removed(var: RustflagsVariable) {
    tracing::debug!(
        var = var.as_str(),
        "removed rustflags variable entirely after coverage flag stripping"
    );
}

/// Sanitizes `RUSTFLAGS` by removing coverage tokens.
///
/// Returns `None` when all tokens are stripped, preserving the original string
/// unchanged when no coverage tokens are present to avoid needless allocation.
fn sanitize_plain(flags: RustflagsValue<'_>) -> Option<String> {
    sanitize(flags, Delimiter::Whitespace)
}

/// Sanitizes `CARGO_ENCODED_RUSTFLAGS` by removing coverage tokens.
///
/// Returns `None` when all tokens are stripped.
fn sanitize_encoded(flags: RustflagsValue<'_>) -> Option<String> {
    sanitize(flags, Delimiter::UnitSeparator)
}

/// How a rustflags value separates its tokens.
///
/// The two variables differ only in this, which is why one sanitizer serves
/// both: the choice also selects the separator the surviving tokens are
/// rejoined with, so a value can never be split on one delimiter and joined
/// on the other.
#[derive(Clone, Copy)]
enum Delimiter {
    /// `RUSTFLAGS`: any run of ASCII whitespace, as `split_whitespace` finds.
    Whitespace,
    /// `CARGO_ENCODED_RUSTFLAGS`: Cargo's unit separator.
    UnitSeparator,
}

impl Delimiter {
    /// Splits `value` into tokens.
    ///
    /// Boxed because the two splitters are different iterator types, and the
    /// sanitizer must not care which one produced its tokens.
    fn split<'a>(
        self,
        value: RustflagsValue<'a>,
    ) -> Box<dyn Iterator<Item = RustflagToken<'a>> + 'a> {
        let text = value.as_str();
        match self {
            Self::Whitespace => Box::new(text.split_whitespace().map(RustflagToken)),
            Self::UnitSeparator => {
                Box::new(text.split(ENCODED_RUSTFLAGS_SEPARATOR).map(RustflagToken))
            }
        }
    }

    /// The separator surviving tokens are rejoined with.
    fn join_separator(self) -> String {
        match self {
            Self::Whitespace => " ".to_owned(),
            Self::UnitSeparator => ENCODED_RUSTFLAGS_SEPARATOR.to_string(),
        }
    }
}

/// Strips coverage tokens from `flags`, rejoining what survives.
fn sanitize(flags: RustflagsValue<'_>, delimiter: Delimiter) -> Option<String> {
    let (tokens, was_changed) = filtered_rustflag_tokens(delimiter.split(flags));
    if was_changed {
        non_empty_join(&tokens, &delimiter.join_separator())
    } else {
        Some(flags.as_str().to_owned())
    }
}

/// Iterates over `tokens`, discarding any that match the
/// `-Cinstrument-coverage` codegen flag in compact and split forms.
///
/// Returns the retained tokens and a boolean indicating whether any token was
/// removed.
fn filtered_rustflag_tokens<'a>(
    tokens: impl IntoIterator<Item = RustflagToken<'a>>,
) -> (Vec<RustflagToken<'a>>, bool) {
    let mut filtered = Vec::new();
    let mut was_changed = false;
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        if is_instrument_coverage_codegen_flag(token) {
            was_changed = true;
            continue;
        }
        if token.as_str() == "-C"
            && iter
                .peek()
                .is_some_and(|next| is_instrument_coverage_option(*next))
        {
            let _ = iter.next();
            was_changed = true;
            continue;
        }
        filtered.push(token);
    }
    (filtered, was_changed)
}

/// Joins `tokens` with `separator` if the slice is non-empty; returns `None`
/// otherwise.
fn non_empty_join(tokens: &[RustflagToken<'_>], separator: &str) -> Option<String> {
    if tokens.is_empty() {
        return None;
    }
    let parts: Vec<&str> = tokens.iter().map(|token| token.as_str()).collect();
    Some(parts.join(separator))
}

/// Returns `true` if `token` is a compact codegen flag
/// (`-Cinstrument-coverage` or `-Cinstrument-coverage=...`).
fn is_instrument_coverage_codegen_flag(token: RustflagToken<'_>) -> bool {
    token
        .as_str()
        .strip_prefix("-C")
        .is_some_and(|rest| is_instrument_coverage_option(RustflagToken(rest)))
}

/// Returns `true` if `token` is the `instrument-coverage` option name used
/// after a split `-C` flag.
fn is_instrument_coverage_option(token: RustflagToken<'_>) -> bool {
    let name = token.as_str();
    name == "instrument-coverage" || name.starts_with("instrument-coverage=")
}

#[cfg(test)]
#[path = "rustflags_proptests.rs"]
mod proptests;

#[cfg(test)]
mod tests {
    //! Unit tests for `RUSTFLAGS` handling.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(
        "  --cfg   caller_gate   -Clink-arg=/SAFESEH:NO  ",
        Some("  --cfg   caller_gate   -Clink-arg=/SAFESEH:NO  ".to_owned())
    )]
    #[case(
        "--cfg caller_gate -Clink-arg=/SAFESEH:NO",
        Some("--cfg caller_gate -Clink-arg=/SAFESEH:NO".to_owned())
    )]
    #[case(
        "-Cinstrument-coverage --cfg caller_gate",
        Some("--cfg caller_gate".to_owned())
    )]
    #[case(
        "-C instrument-coverage --cfg caller_gate",
        Some("--cfg caller_gate".to_owned())
    )]
    #[case("-Cinstrument-coverage", None)]
    fn plain_rustflags_preserve_user_flags_while_stripping_coverage(
        #[case] flags: &str,
        #[case] expected: Option<String>,
    ) {
        assert_eq!(sanitize_plain(RustflagsValue(flags)), expected);
    }

    #[test]
    fn encoded_rustflags_preserve_user_flags_while_stripping_coverage() {
        let separator = ENCODED_RUSTFLAGS_SEPARATOR.to_string();
        let flags = [
            "-Cinstrument-coverage",
            "--cfg",
            "caller_gate",
            "-C",
            "link-arg=/SAFESEH:NO",
        ]
        .join(&separator);
        let expected = ["--cfg", "caller_gate", "-C", "link-arg=/SAFESEH:NO"].join(&separator);

        assert_eq!(sanitize_encoded(RustflagsValue(&flags)), Some(expected));
    }
}
