//! Snapshot tests for localised help output across different locales.
//!
//! Uses `assert_cmd` to run the compiled binary with different `LANG`
//! environment settings and `insta` to snapshot the `--help` output.

use assert_cmd::Command;
use insta::assert_snapshot;

/// Runs the `hello_world` binary with the specified locale and arguments,
/// returning the combined output for snapshot comparison.
fn run_with_locale(locale: &str, args: &[&str]) -> String {
    #[expect(
        deprecated,
        clippy::expect_used,
        reason = "cargo_bin is the standard assert_cmd API and test panics are acceptable"
    )]
    let mut cmd = Command::cargo_bin("hello_world").expect("binary should exist");

    // Clear locale-related env vars to ensure isolation, then set the desired locale
    cmd.env_remove("LC_ALL");
    cmd.env_remove("LC_MESSAGES");
    cmd.env_remove("LANG");
    cmd.env("LANG", locale);

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

    // Normalize CRLF to LF for cross-platform consistency
    combined.replace("\r\n", "\n")
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
    assert!(
        output.contains("OrthoConfig"),
        "expected stock clap text in output: {output}"
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
