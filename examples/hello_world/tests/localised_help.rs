//! Snapshot tests for localised help output across different locales.
//!
//! Uses `assert_cmd` to run the compiled binary with different `LANG`
//! environment settings and `insta` to snapshot the `--help` output.

use assert_cmd::Command;
use insta::assert_snapshot;

/// Runs the `hello_world` binary with the specified locale environment variables
/// and arguments, returning the combined output for snapshot comparison.
///
/// The `locale_env` parameter specifies which locale environment variables to set
/// (e.g., `[("LC_ALL", "ja_JP.UTF-8"), ("LANG", "en_US.UTF-8")]`).
fn run_with_env(locale_env: &[(&str, &str)], args: &[&str]) -> String {
    #[expect(
        deprecated,
        clippy::expect_used,
        reason = "cargo_bin is the standard assert_cmd API and test panics are acceptable"
    )]
    let mut cmd = Command::cargo_bin("hello_world").expect("binary should exist");

    // Clear locale-related env vars to ensure isolation
    cmd.env_remove("LC_ALL");
    cmd.env_remove("LC_MESSAGES");
    cmd.env_remove("LANG");

    // Set the specified locale environment variables
    for (key, value) in locale_env {
        cmd.env(key, value);
    }

    // Disable backtraces to ensure consistent output across environments
    // (CI coverage runs set RUST_BACKTRACE=1 which would include full backtraces)
    cmd.env("RUST_BACKTRACE", "0");

    cmd.args(args);

    #[expect(clippy::expect_used, reason = "test panics are acceptable")]
    let output = cmd.output().expect("command should execute");

    // For help output, clap writes to stdout on success
    // For errors, clap writes to stderr
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Combine output, preferring stdout for help, stderr for errors
    let combined = if stdout.is_empty() {
        stderr.into_owned()
    } else {
        stdout.into_owned()
    };

    // Normalise for cross-platform consistency:
    // - CRLF to LF for line endings
    // - Backslashes to forward slashes for paths (Windows uses backslashes in error output)
    let normalised = combined.replace("\r\n", "\n").replace('\\', "/");
    normalise_rust_src_paths(&normalised)
}

/// Runs the `hello_world` binary with the specified locale (via `LANG`) and arguments,
/// returning the combined output for snapshot comparison.
///
/// This is a convenience wrapper around [`run_with_env`] that only sets `LANG`.
fn run_with_locale(locale: &str, args: &[&str]) -> String {
    run_with_env(&[("LANG", locale)], args)
}

/// Rewrites rustup toolchain source paths to a stable `<rust-src>` prefix.
///
/// This keeps snapshots portable across environments where the absolute rustup
/// installation path differs.
fn normalise_rust_src_paths(output: &str) -> String {
    let marker = "/library/core/src/ops/function.rs";
    let mut normalised = output
        .lines()
        .map(|line| {
            let trimmed = line.trim_start_matches(' ');
            trimmed
                .find(marker)
                .and_then(|pos| trimmed.get(pos..))
                .map_or_else(
                    || line.to_owned(),
                    |suffix| {
                        let indent_len = line.len() - trimmed.len();
                        let indent = " ".repeat(indent_len);
                        format!("{indent}<rust-src>{suffix}")
                    },
                )
        })
        .collect::<Vec<_>>()
        .join("\n");
    if output.ends_with('\n') {
        normalised.push('\n');
    }
    normalised
}

#[test]
fn normalise_rust_src_paths_rewrites_only_matching_lines() {
    let input = concat!(
        "error: panic\n",
        "  /Users/example/.rustup/toolchains/stable/library/core/src/ops/function.rs:10:9\n",
        "no marker here"
    );
    let expected = concat!(
        "error: panic\n",
        "  <rust-src>/library/core/src/ops/function.rs:10:9\n",
        "no marker here"
    );
    assert_eq!(normalise_rust_src_paths(input), expected);
}

// =============================================================================
// English (en-US) help output tests
// =============================================================================

#[test]
fn help_en_us() {
    let output = run_with_locale("en_US.UTF-8", &["--help"]);
    assert_snapshot!(output);
}

#[test]
fn greet_help_en_us() {
    let output = run_with_locale("en_US.UTF-8", &["greet", "--help"]);
    assert_snapshot!(output);
}

#[test]
fn take_leave_help_en_us() {
    let output = run_with_locale("en_US.UTF-8", &["take-leave", "--help"]);
    assert_snapshot!(output);
}

#[test]
fn missing_subcommand_error_en_us() {
    let output = run_with_locale("en_US.UTF-8", &[]);
    assert_snapshot!(output);
}

// =============================================================================
// Japanese (ja) help output tests
// =============================================================================

#[test]
fn help_ja() {
    let output = run_with_locale("ja_JP.UTF-8", &["--help"]);
    assert_snapshot!(output);
}

#[test]
fn greet_help_ja() {
    let output = run_with_locale("ja_JP.UTF-8", &["greet", "--help"]);
    assert_snapshot!(output);
}

#[test]
fn take_leave_help_ja() {
    let output = run_with_locale("ja_JP.UTF-8", &["take-leave", "--help"]);
    assert_snapshot!(output);
}

#[test]
fn missing_subcommand_error_ja() {
    let output = run_with_locale("ja_JP.UTF-8", &[]);
    assert_snapshot!(output);
}

// =============================================================================
// Fallback behaviour tests
// =============================================================================

#[test]
fn fallback_to_english_for_unknown_locale() {
    // Unknown locale should fall back to stock clap strings gracefully
    let output = run_with_locale("xx_YY.UTF-8", &["--help"]);
    // Should contain stock English text (original clap about), not crash or show garbage
    // When locale is unsupported, we fall back to NoOpLocalizer which preserves clap defaults

    // Assert presence of English-only text that doesn't appear in Japanese
    assert!(
        output.contains("OrthoConfig"),
        "expected stock clap text in output: {output}"
    );

    // Assert absence of Japanese text to confirm we're not accidentally using Japanese
    assert!(
        !output.contains("挨拶"),
        "Japanese text should not appear for unknown locale: {output}"
    );
    assert!(
        !output.contains("ワークフロー"),
        "Japanese text should not appear for unknown locale: {output}"
    );
}

#[test]
fn c_locale_uses_english() {
    // C locale should be treated as English
    let output = run_with_locale("C", &["--help"]);
    assert!(
        output.contains("layered greetings"),
        "expected English text for C locale: {output}"
    );
}

#[test]
fn posix_locale_uses_english() {
    // POSIX locale should be treated as English
    let output = run_with_locale("POSIX", &["--help"]);
    assert!(
        output.contains("layered greetings"),
        "expected English text for POSIX locale: {output}"
    );
}

// =============================================================================
// Locale environment variable precedence tests
// =============================================================================

/// Asserts that the locale precedence rules produce output containing the expected
/// substring when the given environment variables are set.
fn assert_locale_precedence(
    env_vars: &[(&str, &str)],
    expected_substring: &str,
    description: &str,
) {
    let output = run_with_env(env_vars, &["--help"]);
    assert!(
        output.contains(expected_substring),
        "{description}, got: {output}"
    );
}

#[test]
fn lc_all_takes_precedence_over_lang() {
    // LC_ALL should override LANG
    // Output should be Japanese (from LC_ALL) even though LANG is en_US
    assert_locale_precedence(
        &[("LC_ALL", "ja_JP.UTF-8"), ("LANG", "en_US.UTF-8")],
        "挨拶",
        "expected Japanese text when LC_ALL=ja",
    );
}

#[test]
fn lc_messages_takes_precedence_over_lang() {
    // LC_MESSAGES should override LANG (when LC_ALL is not set)
    // Output should be Japanese (from LC_MESSAGES) even though LANG is en_US
    assert_locale_precedence(
        &[("LC_MESSAGES", "ja_JP.UTF-8"), ("LANG", "en_US.UTF-8")],
        "挨拶",
        "expected Japanese text when LC_MESSAGES=ja",
    );
}

#[test]
fn lc_all_takes_precedence_over_lc_messages() {
    // LC_ALL should override both LC_MESSAGES and LANG
    // Output should be English (from LC_ALL) even though LC_MESSAGES and LANG are Japanese
    assert_locale_precedence(
        &[
            ("LC_ALL", "en_US.UTF-8"),
            ("LC_MESSAGES", "ja_JP.UTF-8"),
            ("LANG", "ja_JP.UTF-8"),
        ],
        "layered greetings",
        "expected English text when LC_ALL=en",
    );
}
