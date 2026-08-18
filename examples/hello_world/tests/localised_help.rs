//! Snapshot tests for localised help output across different locales.
//!
//! Uses `assert_cmd` to run the compiled binary with different `LANG`
//! environment settings and `insta` to snapshot the `--help` output.

use assert_cmd::Command as AssertCommand;
use clap::CommandFactory;
use hello_world::cli::{CommandLine, LocalizeCmd};
use hello_world::localizer::DemoLocalizer;
use insta::assert_snapshot;
use ortho_config::LanguageIdentifier;
use ortho_config::NoOpLocalizer;
use ortho_config::langid;
use rstest::rstest;

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
    let mut cmd = AssertCommand::cargo_bin("hello_world").expect("binary should exist");

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

fn assert_display_request_succeeds(locale: &str, args: &[&str]) {
    #[expect(
        deprecated,
        clippy::expect_used,
        reason = "cargo_bin is the standard assert_cmd API and test panics are acceptable"
    )]
    let mut cmd = AssertCommand::cargo_bin("hello_world").expect("binary should exist");
    cmd.env_remove("LC_ALL");
    cmd.env_remove("LC_MESSAGES");
    cmd.env_remove("LANG");
    cmd.env("LANG", locale);
    cmd.env("RUST_BACKTRACE", "0");
    cmd.args(args);

    #[expect(clippy::expect_used, reason = "test panics are acceptable")]
    let output = cmd.output().expect("command should execute");

    assert!(
        output.status.success(),
        "display request should exit successfully: {output:?}"
    );
    assert!(
        !output.stdout.is_empty(),
        "display request should write to stdout: {output:?}"
    );
    assert!(
        output.stderr.is_empty(),
        "display request should not write to stderr: {output:?}"
    );
}

fn render_localized_long_help(localizer: &dyn ortho_config::Localizer) -> String {
    let mut command = CommandLine::command()
        .with_base("hello_world.cli")
        .localize(localizer);
    command.render_long_help().to_string()
}

fn render_stock_long_help() -> String {
    let mut command = CommandLine::command();
    command.render_long_help().to_string()
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

// =============================================================================
// Direct command-tree localization snapshots
// =============================================================================

/// Renders the localised command tree for `locale` and compares it with the
/// snapshot recorded under `snapshot_name`.
///
/// `rstest` wraps a parameterised test in a module named after the test
/// function, and insta would otherwise fold that module name into the
/// generated snapshot file name. Suppressing the module prefix and passing the
/// full historical name keeps the committed `.snap` files stable.
fn assert_command_tree_snapshot(snapshot_name: &str, locale: LanguageIdentifier) {
    let locale_name = locale.to_string();
    let localizer = match DemoLocalizer::try_for_locale(locale) {
        Ok(localizer) => localizer,
        Err(error) => panic!("{locale_name} demo localizer should build: {error}"),
    };
    let output = render_localized_long_help(&localizer);
    insta::with_settings!({prepend_module_to_snapshot => false}, {
        assert_snapshot!(format!("localised_help__{snapshot_name}"), output);
    });
}

#[rstest]
#[case::en_us("command_tree_long_help_en_us", langid!("en-US"))]
#[case::ja("command_tree_long_help_ja", langid!("ja"))]
fn command_tree_long_help_matches_locale_snapshot(
    #[case] snapshot_name: &str,
    #[case] locale: LanguageIdentifier,
) {
    assert_command_tree_snapshot(snapshot_name, locale);
}

#[test]
fn command_tree_long_help_noop_matches_stock() {
    let output = render_localized_long_help(&NoOpLocalizer::new());
    assert_eq!(output, render_stock_long_help());
    assert_snapshot!("command_tree_long_help_noop", output);
}

#[rstest]
#[case(
    "rewrites_only_matching_lines",
    concat!(
        "error: panic\n",
        "  /Users/example/.rustup/toolchains/stable/library/core/src/ops/function.rs:10:9\n",
        "no marker here"
    ),
    concat!(
        "error: panic\n",
        "  <rust-src>/library/core/src/ops/function.rs:10:9\n",
        "no marker here"
    )
)]
#[case(
    "preserves_trailing_newline",
    concat!(
        "error: panic\n",
        "  /Users/example/.rustup/toolchains/stable/library/core/src/ops/function.rs:10:9\n",
    ),
    concat!(
        "error: panic\n",
        "  <rust-src>/library/core/src/ops/function.rs:10:9\n",
    )
)]
#[expect(
    clippy::used_underscore_binding,
    reason = "The test case label parameter is intentionally named `_desc`."
)]
fn normalise_rust_src_paths_works_correctly(
    #[case] _desc: &str,
    #[case] input: &str,
    #[case] expected: &str,
) {
    assert_eq!(normalise_rust_src_paths(input), expected);
}

// =============================================================================
// English (en-US) help output tests
// =============================================================================

#[test]
fn help_en_us() {
    assert_display_request_succeeds("en_US.UTF-8", &["--help"]);
    assert_display_request_succeeds("en_US.UTF-8", &["--version"]);
    let output = run_with_locale("en_US.UTF-8", &["--help"]);
    assert_snapshot!(output);
}

// =============================================================================
// Japanese (ja) help output tests
// =============================================================================

#[test]
fn help_ja() {
    assert_display_request_succeeds("ja_JP.UTF-8", &["--help"]);
    assert_display_request_succeeds("ja_JP.UTF-8", &["--version"]);
    let output = run_with_locale("ja_JP.UTF-8", &["--help"]);
    assert_snapshot!(output);
}

// =============================================================================
// Per-subcommand help snapshots across locales
// =============================================================================

/// Runs the binary under `locale` and compares the output with the snapshot
/// recorded under `snapshot_name`.
///
/// `rstest` wraps a parameterised test in a module named after the test
/// function, and insta would otherwise fold that module name into the
/// generated snapshot file name. Suppressing the module prefix and passing the
/// full historical name keeps the committed `.snap` files stable.
fn assert_locale_help_snapshot(snapshot_name: &str, locale: &str, args: &[&str]) {
    let output = run_with_locale(locale, args);
    insta::with_settings!({prepend_module_to_snapshot => false}, {
        assert_snapshot!(format!("localised_help__{snapshot_name}"), output);
    });
}

#[rstest]
#[case::greet_help_en_us("greet_help_en_us", "en_US.UTF-8", &["greet", "--help"])]
#[case::take_leave_help_en_us(
    "take_leave_help_en_us",
    "en_US.UTF-8",
    &["take-leave", "--help"]
)]
#[case::missing_subcommand_error_en_us(
    "missing_subcommand_error_en_us",
    "en_US.UTF-8",
    &[] as &[&str]
)]
#[case::greet_help_ja("greet_help_ja", "ja_JP.UTF-8", &["greet", "--help"])]
#[case::take_leave_help_ja("take_leave_help_ja", "ja_JP.UTF-8", &["take-leave", "--help"])]
#[case::missing_subcommand_error_ja(
    "missing_subcommand_error_ja",
    "ja_JP.UTF-8",
    &[] as &[&str]
)]
fn subcommand_help_matches_locale_snapshot(
    #[case] snapshot_name: &str,
    #[case] locale: &str,
    #[case] args: &[&str],
) {
    assert_locale_help_snapshot(snapshot_name, locale, args);
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

/// The `C` and `POSIX` locales should both be treated as English.
#[rstest]
#[case::c_locale("C")]
#[case::posix_locale("POSIX")]
fn portable_locale_uses_english(#[case] locale: &str) {
    let output = run_with_locale(locale, &["--help"]);
    assert!(
        output.contains("layered greetings"),
        "expected English text for {locale} locale: {output}"
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

/// `LC_ALL` outranks `LC_MESSAGES`, which in turn outranks `LANG`.
#[rstest]
// Output should be Japanese (from LC_ALL) even though LANG is en_US.
#[case::lc_all_over_lang(
    &[("LC_ALL", "ja_JP.UTF-8"), ("LANG", "en_US.UTF-8")],
    "挨拶",
    "expected Japanese text when LC_ALL=ja"
)]
// LC_MESSAGES should override LANG when LC_ALL is not set.
#[case::lc_messages_over_lang(
    &[("LC_MESSAGES", "ja_JP.UTF-8"), ("LANG", "en_US.UTF-8")],
    "挨拶",
    "expected Japanese text when LC_MESSAGES=ja"
)]
// LC_ALL should override both LC_MESSAGES and LANG.
#[case::lc_all_over_lc_messages(
    &[
        ("LC_ALL", "en_US.UTF-8"),
        ("LC_MESSAGES", "ja_JP.UTF-8"),
        ("LANG", "ja_JP.UTF-8"),
    ],
    "layered greetings",
    "expected English text when LC_ALL=en"
)]
fn locale_env_vars_follow_precedence(
    #[case] env_vars: &[(&str, &str)],
    #[case] expected_substring: &str,
    #[case] description: &str,
) {
    assert_locale_precedence(env_vars, expected_substring, description);
}
