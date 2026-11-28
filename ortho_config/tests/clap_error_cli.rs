//! End-to-end checks for clap error streams, exit status, and translation.
//!
//! Each case runs the `clap-error-fixture` workspace binary, so these
//! assertions cover the process-visible contract (stdout, stderr, and exit
//! status) rather than `clap`'s in-process API. The locale is injected into the
//! child process, which keeps this suite free of global environment mutation.

use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use test_helpers::process::run_with_deadline;

#[path = "support/clap_error_fixture_lock.rs"]
mod fixture_build_lock;

// The translated-detail cases live in their own file so this target stays
// within the repository's line limit.
#[path = "clap_error_cli/translated_context.rs"]
mod translated_context;

/// The fixture binary name, without a platform executable suffix.
const FIXTURE_BIN: &str = "clap-error-fixture";

/// The workspace package that owns the fixture binary.
const FIXTURE_PACKAGE: &str = "clap_error_matrix_fixture";

/// Budget for the whole one-time fixture build phase.
///
/// The budget covers waiting for another test process's build as well as
/// running Cargo itself, because nextest gives each test process its own
/// 960-second allowance and a wait charged to nobody would let the build
/// overrun it. 600 seconds leaves the fixture runs that follow in the same test
/// a comfortable remainder.
const FIXTURE_BUILD_DEADLINE: Duration = Duration::from_secs(600);

/// Budget for one fixture run.
const FIXTURE_RUN_DEADLINE: Duration = Duration::from_secs(30);

/// Guards the fixture build within one test process.
///
/// nextest runs every test in its own process, so this cache saves work only
/// within a process; the advisory lock in [`fixture_build_lock::acquire`] is
/// what prevents concurrent builds across processes.
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
/// cloneable. The build itself runs under an advisory lock, because nextest's
/// per-process execution would otherwise let several cases nest `cargo` at once.
fn build_fixture() -> std::result::Result<PathBuf, String> {
    let started = Instant::now();
    let _build_lock = fixture_build_lock::acquire(started, FIXTURE_BUILD_DEADLINE)
        .map_err(|error| format!("the fixture build lock should be acquired: {error:#}"))?;
    let remaining = FIXTURE_BUILD_DEADLINE.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Err(format!(
            "the fixture build budget of {FIXTURE_BUILD_DEADLINE:?} elapsed while another process held the build lock"
        ));
    }

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
    let output = run_with_deadline(command, remaining, "clap error fixture build")
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

/// The lock file shared by every process of this suite.
///
/// Every test process of a run executes the same binary, so the directory
/// holding it is the one location they agree on without guessing at Cargo's
/// target layout, which differs between `cargo test` and `cargo nextest` and
/// moves with `--target-dir` and cross-compilation. The file itself is an empty
/// advisory-lock target; nothing writes to it.
fn build_lock_path() -> Result<PathBuf> {
    let executable = std::env::current_exe().context("locate this test executable")?;
    let directory = executable.parent().with_context(|| {
        format!(
            "{} should sit in the directory that holds the test binaries",
            executable.display()
        )
    })?;
    Ok(directory.join("clap-error-fixture-build.lock"))
}

/// A second acquisition waits for a held lock instead of building alongside it.
///
/// Each acquisition opens its own descriptor, which is exactly the situation
/// of two test processes contending for one build; dropping the lock from
/// [`fixture_build_lock::acquire_at`] would let this case succeed immediately
/// rather than report the elapsed wait budget.
///
/// The lock lives in a directory this test owns. Contending on the suite's
/// shared lock would put the outcome at the mercy of a sibling case's build,
/// which is permitted to hold that lock for its whole 600-second budget rather
/// than the 200 milliseconds under test.
#[test]
fn a_held_build_lock_blocks_a_second_acquisition() {
    use std::sync::mpsc;

    // The assertions below are the outcome under test, so this case reports
    // through them rather than through `Result`, which the workspace forbids a
    // panicking function from returning.
    let directory = TempDir::new().expect("create a directory for the lock file");
    let path = directory.path().join("fixture-build.lock");
    let held_path = path.clone();

    let (ready, ready_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            // Hold the lock under the same budget the real build path uses: a
            // sibling process may still be building, and the wait for its lock
            // is part of the build phase rather than of one fixture run.
            let lock =
                fixture_build_lock::acquire_at(&held_path, Instant::now(), FIXTURE_BUILD_DEADLINE)
                    .expect("the first acquisition should succeed");
            ready.send(()).expect("the test should still be waiting");
            release_rx.recv().expect("the test should release the lock");
            drop(lock);
        });
        ready_rx
            .recv()
            .expect("the first acquisition should report readiness");

        // The attempt resolves before the holder is released, so every
        // expectation below runs with the sender already used: a regression
        // whose assertion fails reports that failure instead of leaving the
        // holder blocked in `recv` while the scope waits for it to finish.
        let attempt =
            fixture_build_lock::acquire_at(&path, Instant::now(), Duration::from_millis(200));
        release
            .send(())
            .expect("the holder should still be waiting");

        let error = attempt.expect_err("a held lock must block a second acquisition");
        assert!(
            error
                .to_string()
                .contains("stayed held past its 200ms budget"),
            "the second acquisition should report the budget it was given, got: {error}"
        );
    });
}

/// Finds the fixture's executable in Cargo's artefact records.
fn reported_executable(build_output: &[u8]) -> Option<PathBuf> {
    String::from_utf8_lossy(build_output)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(is_fixture_artifact)
        .find_map(|record| record.get("executable")?.as_str().map(PathBuf::from))
}

/// Whether a record is a compiler artefact for the fixture binary.
fn is_fixture_artifact(record: &Value) -> bool {
    is_compiler_artifact(record) && names_fixture_bin(record)
}

/// Whether a record reports compiler output rather than a build message.
fn is_compiler_artifact(record: &Value) -> bool {
    record.get("reason").and_then(Value::as_str) == Some("compiler-artifact")
}

/// Whether a record's target is the fixture binary.
fn names_fixture_bin(record: &Value) -> bool {
    record.pointer("/target/name").and_then(Value::as_str) == Some(FIXTURE_BIN)
        && target_is_binary(record)
}

/// Whether a record's target kind list includes a binary.
fn target_is_binary(record: &Value) -> bool {
    record
        .pointer("/target/kind")
        .and_then(Value::as_array)
        .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("bin")))
}

/// Runs the fixture with `args`, injecting `locale` and disabling colour.
///
/// The child starts with an empty environment so the only inputs are the ones
/// named here: a variable inherited from the developer's shell cannot change
/// the rendering under test. The mutation is confined to the child process,
/// which is the sole exemption to this repository's injection rule.
fn run_fixture(locale: &str, args: &[&str]) -> Result<Output> {
    let mut command = Command::new(fixture_binary()?);
    command
        .args(args)
        .env_clear()
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
