//! End-to-end checks for clap error streams, exit status, and translation.
//!
//! Each case runs the `clap-error-fixture` workspace binary, so these
//! assertions cover the process-visible contract (stdout, stderr, and exit
//! status) rather than `clap`'s in-process API. The locale is injected into the
//! child process, which keeps this suite free of global environment mutation.

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

/// The fixture binary name, without a platform executable suffix.
const FIXTURE_BIN: &str = "clap-error-fixture";

/// The workspace package that owns the fixture binary.
const FIXTURE_PACKAGE: &str = "clap_error_matrix_fixture";

/// Guards the one-time fixture build.
static BUILD_FIXTURE: OnceLock<std::result::Result<(), String>> = OnceLock::new();

/// Builds the fixture once per test process.
///
/// The fixture is a separate workspace member, so a targeted run of this suite
/// cannot rely on `cargo test --workspace` having built it already.
fn ensure_fixture_built() -> Result<()> {
    match BUILD_FIXTURE.get_or_init(build_fixture) {
        Ok(()) => Ok(()),
        Err(message) => Err(anyhow!("{message}")),
    }
}

/// Performs the one-time build, reporting a failure as text.
///
/// The outcome is stored in a [`OnceLock`] shared by every test thread, so it
/// carries a message rather than the underlying error type, which is not
/// cloneable.
fn build_fixture() -> std::result::Result<(), String> {
    let status = Command::new("cargo")
        .args(["build", "--quiet", "-p", FIXTURE_PACKAGE])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .map_err(|error| format!("cargo should start to build the fixture: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("the fixture build should succeed, got {status}"))
    }
}

/// Locates the fixture binary in this test executable's profile directory.
///
/// Walking up from `target/<profile>/deps/<test>` keeps custom `--target-dir`
/// and cross-target layouts correct without re-deriving Cargo's configuration.
fn fixture_binary() -> Result<PathBuf> {
    let test_binary = std::env::current_exe().context("the test executable path should resolve")?;
    let profile_dir = test_binary
        .parent()
        .and_then(Path::parent)
        .context("the test executable should live in a profile directory")?;
    Ok(profile_dir.join(format!("{FIXTURE_BIN}{}", std::env::consts::EXE_SUFFIX)))
}

/// Runs the fixture with `args`, injecting `locale` and disabling colour.
fn run_fixture(locale: &str, args: &[&str]) -> Result<Output> {
    ensure_fixture_built()?;
    let output = Command::new(fixture_binary()?)
        .args(args)
        .env("CLAP_ERROR_FIXTURE_LOCALE", locale)
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .output()
        .context("the fixture should run")?;
    Ok(output)
}

/// Returns the child's standard output as text.
fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// Returns the child's standard error as text.
fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Help is clap's own surface: stdout, success, and untouched by locale.
#[test]
fn help_is_displayed_on_stdout_and_ignores_the_locale() {
    let en_us = run_fixture("en-US", &["--help"]).expect("the fixture should run");
    let other = run_fixture("fr-FR", &["--help"]).expect("the fixture should run");

    assert_eq!(
        en_us.status.code(),
        Some(0),
        "help should exit successfully"
    );
    let text = stdout_of(&en_us);
    assert!(
        text.contains("Usage: clap-error-fixture"),
        "help should render usage on stdout, got: {text}"
    );
    assert!(
        stderr_of(&en_us).is_empty(),
        "help must not write to stderr, got: {}",
        stderr_of(&en_us)
    );
    assert_eq!(
        en_us.stdout, other.stdout,
        "display-only help must not change with the locale"
    );
    assert_eq!(other.status.code(), Some(0));
}

/// Version is clap's own surface: stdout, success, and untouched by locale.
#[test]
fn version_is_displayed_on_stdout() {
    let output = run_fixture("en-US", &["--version"]).expect("the fixture should run");

    assert_eq!(
        output.status.code(),
        Some(0),
        "version should exit successfully"
    );
    assert_eq!(stdout_of(&output).trim(), "clap-error-fixture 1.2.3");
    assert!(
        stderr_of(&output).is_empty(),
        "version must not write to stderr, got: {}",
        stderr_of(&output)
    );
}

/// Missing-input help stays clap output even when a catalogue is available.
#[test]
fn missing_input_help_keeps_clap_text_and_stream() {
    let output = run_fixture("en-US", &[]).expect("the fixture should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "missing-input help should fail with clap's usage exit code"
    );
    assert!(
        stdout_of(&output).is_empty(),
        "missing-input help should not write to stdout"
    );
    let text = stderr_of(&output);
    assert!(
        text.contains("Usage: clap-error-fixture"),
        "expected clap's rendered help on stderr, got: {text}"
    );
    assert!(
        !text.contains("Equal sign is required"),
        "display-only help must not be replaced by a translation, got: {text}"
    );
}

/// A newly covered parser error is translated on stderr.
#[test]
fn no_equals_error_is_translated_on_stderr() {
    let output = run_fixture("en-US", &["--colour", "red"]).expect("the fixture should run");

    assert_eq!(
        output.status.code(),
        Some(2),
        "parser errors should keep clap's usage exit code"
    );
    assert!(
        stdout_of(&output).is_empty(),
        "parser errors should not write to stdout"
    );
    let text = stderr_of(&output);
    assert!(
        text.contains("Equal sign is required"),
        "expected the en-US translation, got: {text}"
    );
    assert!(
        text.contains("--colour"),
        "expected the offending argument in the translation, got: {text}"
    );
    assert!(
        !text.contains('{'),
        "no placeholder should remain unresolved, got: {text}"
    );
}

/// A locale without the message keeps clap's diagnostic and stream.
#[test]
fn unavailable_locale_retains_the_stock_diagnostic() {
    let stock = run_fixture("fr-FR", &["--colour", "red"]).expect("the fixture should run");
    let translated = run_fixture("en-US", &["--colour", "red"]).expect("the fixture should run");

    assert_eq!(
        stock.status.code(),
        Some(2),
        "the fallback should keep clap's exit code"
    );
    assert!(
        stdout_of(&stock).is_empty(),
        "the fallback should keep clap's stderr routing"
    );
    let stock_text = stderr_of(&stock);
    assert!(
        stock_text.contains("equal sign is needed"),
        "expected clap's stock diagnostic, got: {stock_text}"
    );
    assert_ne!(
        stock_text,
        stderr_of(&translated),
        "an unavailable translation should not mimic the translated text"
    );
}
