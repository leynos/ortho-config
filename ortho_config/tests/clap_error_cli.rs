//! End-to-end checks for clap error streams, exit status, and translation.
//!
//! Each case runs the `clap-error-fixture` workspace binary, so these
//! assertions cover the process-visible contract (stdout, stderr, and exit
//! status) rather than `clap`'s in-process API. The locale is injected into the
//! child process, which keeps this suite free of global environment mutation.

use anyhow::{Context, Result, anyhow};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::Duration;
use test_helpers::process::run_with_deadline;

/// The fixture binary name, without a platform executable suffix.
const FIXTURE_BIN: &str = "clap-error-fixture";

/// The workspace package that owns the fixture binary.
const FIXTURE_PACKAGE: &str = "clap_error_matrix_fixture";

/// Budget for the one-time nested fixture build.
const FIXTURE_BUILD_DEADLINE: Duration = Duration::from_secs(240);

/// Budget for one fixture run.
const FIXTURE_RUN_DEADLINE: Duration = Duration::from_secs(30);

/// Guards the one-time fixture build and the executable it reported.
static FIXTURE_BINARY: OnceLock<std::result::Result<PathBuf, String>> = OnceLock::new();

/// Returns the fixture executable, building it once per test process.
///
/// The fixture is a separate workspace member, so a targeted run of this suite
/// cannot rely on `cargo test --workspace` having built it already.
fn fixture_binary() -> Result<&'static Path> {
    match FIXTURE_BINARY.get_or_init(build_fixture) {
        Ok(path) => Ok(path),
        Err(message) => Err(anyhow!("{message}")),
    }
}

/// Builds the fixture and reports the executable Cargo produced.
///
/// The executable is read back from Cargo's JSON artefact stream rather than
/// derived from this test's own location. A nested `cargo build` selects its
/// own profile and target directory, which need not match the test binary's
/// under `cargo test --release`, a custom `--profile`, or a cross-target run.
///
/// The outcome is stored in a [`OnceLock`] shared by every test thread, so it
/// carries a message rather than the underlying error type, which is not
/// cloneable.
fn build_fixture() -> std::result::Result<PathBuf, String> {
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "--message-format",
            "json-render-diagnostics",
            "-p",
            FIXTURE_PACKAGE,
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"));
    let output = run_with_deadline(command, FIXTURE_BUILD_DEADLINE, "clap error fixture build")
        .map_err(|error| format!("the fixture build should finish: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "the fixture build should succeed, got {status}\n--- stderr ---\n{stderr}",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr),
        ));
    }
    reported_executable(&output.stdout)
        .ok_or_else(|| "the fixture build should report its executable".to_owned())
}

/// Finds the fixture's executable in Cargo's artefact records.
fn reported_executable(build_output: &[u8]) -> Option<PathBuf> {
    String::from_utf8_lossy(build_output)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|record| {
            let target = record.get("target")?;
            let is_binary = target
                .get("kind")?
                .as_array()?
                .iter()
                .any(|kind| kind.as_str() == Some("bin"));
            if record.get("reason")?.as_str()? != "compiler-artifact"
                || !is_binary
                || target.get("name")?.as_str()? != FIXTURE_BIN
            {
                return None;
            }
            Some(PathBuf::from(record.get("executable")?.as_str()?))
        })
}

/// Runs the fixture with `args`, injecting `locale` and disabling colour.
fn run_fixture(locale: &str, args: &[&str]) -> Result<Output> {
    let mut command = Command::new(fixture_binary()?);
    command
        .args(args)
        .env("CLAP_ERROR_FIXTURE_LOCALE", locale)
        .env("NO_COLOR", "1");
    run_with_deadline(command, FIXTURE_RUN_DEADLINE, "clap error fixture run")
        .with_context(|| format!("run the fixture with locale {locale}"))
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
