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

const ENCODED_RUSTFLAGS_SEPARATOR: char = '\x1f';

/// Applies sanitized Rust compiler flags to `command`.
///
/// Reads `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` from the current process
/// environment, strips any `-Cinstrument-coverage` tokens injected by
/// `cargo-llvm-cov`, and re-applies the sanitized values to `command`.
/// Variables that become empty after sanitization are removed entirely so the
/// child process inherits a clean flag set rather than an empty string.
pub(crate) fn apply_sanitized_rustflags(command: &mut Command) {
    apply_sanitized_rustflags_var(command, "RUSTFLAGS", sanitize_plain_rustflags);
    apply_sanitized_rustflags_var(
        command,
        "CARGO_ENCODED_RUSTFLAGS",
        sanitize_encoded_rustflags,
    );
}

/// Reads the named environment variable, sanitizes it with `sanitize`, and
/// either sets the sanitized value on `command` or removes the variable when
/// sanitization produces no tokens.
fn apply_sanitized_rustflags_var(
    command: &mut Command,
    name: &str,
    sanitize: fn(&str) -> Option<String>,
) {
    let Ok(value) = std::env::var(name) else {
        return;
    };
    match sanitize(&value) {
        Some(sanitized) if sanitized == value => {
            // No coverage tokens found; pass through unchanged.
            command.env(name, sanitized);
        }
        Some(sanitized) => {
            tracing::debug!(
                var = name,
                original = value,
                sanitized = sanitized,
                "stripped coverage flags from rustflags variable"
            );
            command.env(name, sanitized);
        }
        None => {
            tracing::debug!(
                var = name,
                original = value,
                "removed rustflags variable entirely after coverage flag stripping"
            );
            command.env_remove(name);
        }
    }
}

/// Sanitizes whitespace-delimited `RUSTFLAGS` by removing coverage tokens.
///
/// Returns `None` when all tokens are stripped, preserving the original string
/// unchanged when no coverage tokens are present to avoid needless allocation.
fn sanitize_plain_rustflags(flags: &str) -> Option<String> {
    let (tokens, was_changed) = filtered_rustflag_tokens(flags.split_whitespace());
    if was_changed {
        non_empty_join(&tokens, " ")
    } else {
        Some(flags.to_owned())
    }
}

/// Sanitizes unit-separator-delimited `CARGO_ENCODED_RUSTFLAGS` by removing
/// coverage tokens.
///
/// Returns `None` when all tokens are stripped.
fn sanitize_encoded_rustflags(flags: &str) -> Option<String> {
    let (tokens, was_changed) = filtered_rustflag_tokens(flags.split(ENCODED_RUSTFLAGS_SEPARATOR));
    if was_changed {
        non_empty_join(&tokens, &ENCODED_RUSTFLAGS_SEPARATOR.to_string())
    } else {
        Some(flags.to_owned())
    }
}

/// Iterates over `tokens`, discarding any that match the
/// `-Cinstrument-coverage` codegen flag in compact and split forms.
///
/// Returns the retained tokens and a boolean indicating whether any token was
/// removed.
fn filtered_rustflag_tokens<'a>(tokens: impl IntoIterator<Item = &'a str>) -> (Vec<&'a str>, bool) {
    let mut filtered = Vec::new();
    let mut was_changed = false;
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        if is_instrument_coverage_codegen_flag(token) {
            was_changed = true;
            continue;
        }
        if token == "-C"
            && iter
                .peek()
                .is_some_and(|next| is_instrument_coverage_option(next))
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
fn non_empty_join(tokens: &[&str], separator: &str) -> Option<String> {
    (!tokens.is_empty()).then(|| tokens.join(separator))
}

/// Returns `true` if `token` is a compact codegen flag
/// (`-Cinstrument-coverage` or `-Cinstrument-coverage=...`).
fn is_instrument_coverage_codegen_flag(token: &str) -> bool {
    token
        .strip_prefix("-C")
        .is_some_and(is_instrument_coverage_option)
}

/// Returns `true` if `token` is the `instrument-coverage` option name used
/// after a split `-C` flag.
fn is_instrument_coverage_option(token: &str) -> bool {
    token == "instrument-coverage" || token.starts_with("instrument-coverage=")
}

#[cfg(test)]
mod tests {
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
        assert_eq!(sanitize_plain_rustflags(flags), expected);
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

        assert_eq!(sanitize_encoded_rustflags(&flags), Some(expected));
    }
}
