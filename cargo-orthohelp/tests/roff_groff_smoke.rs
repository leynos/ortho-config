//! Real-processor smoke test for the roff renderer.
//!
//! The unit and golden tests assert the bytes the encoder produces. This test
//! closes the loop by handing an adversarial page to an actual roff processor,
//! so "a metadata value cannot become a request" is checked by the
//! implementation that defines roff rather than by our reading of it.
//!
//! A man page processor is not a test dependency: `groff` is installed on most
//! Linux developer hosts and `mandoc` on BSD ones, but neither is present
//! everywhere. Each test runs its processor directly and skips only when the
//! program is not installed, so an installed processor is always exercised.
//!
//! Every command runs through `ortho_config_test_helpers::process`, as the
//! design document requires for test-only commands that start a process, so a
//! hung processor is bounded by a deadline and has its process tree cleaned
//! up. A missing program is reported by the helper as a not-found spawn error,
//! which is the only failure that means "skip".

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use cargo_orthohelp::ir::LocalizedLink;
use cargo_orthohelp::roff::{RoffConfig, generate_to_string};
use std::error::Error;
use std::io::Write;
use std::process::{Command, Output};
use std::time::Duration;
use test_helpers::process::run_with_deadline;

#[path = "fixtures/nested_fixture.rs"]
mod nested_fixture;

/// Path the `.so` payload names; a processor that executes it reports the
/// path, which makes the injection visible without needing the file to exist.
const INJECTED_SO_TARGET: &str = "/nonexistent-orthohelp-so-probe";

/// Marker the `.tm` payload prints when a processor executes it.
const INJECTED_TM_MARKER: &str = "ROFF-INJECTION-MARKER";

/// Deadline for formatting one page. A loaded host can slow a process start,
/// but a processor that has not finished in five minutes is hung.
const FORMAT_DEADLINE: Duration = Duration::from_secs(300);

/// Runs `command`, returning `None` only when the program is not installed.
///
/// A missing processor gives this test nothing to drive, so it skips. Every
/// other failure -- a deadline, a cleanup failure, or a program that exists
/// but cannot be executed -- is returned and fails the test.
fn run_optional(
    command: Command,
    deadline: Duration,
    description: &str,
) -> anyhow::Result<Option<Output>> {
    match run_with_deadline(command, deadline, description) {
        Ok(output) => Ok(Some(output)),
        Err(error) if is_missing_program(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Returns whether the run failed because the program is not installed.
fn is_missing_program(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_error| io_error.kind() == std::io::ErrorKind::NotFound)
    })
}

/// Builds a page whose every prose sink carries an injection payload.
///
/// The command name stays clean so the rendered output still has a visible
/// title, which proves the processor formatted the page rather than rejecting
/// it.
fn adversarial_page() -> String {
    let payload = format!("value\r\u{0}.tm {INJECTED_TM_MARKER}\n\u{0}.so {INJECTED_SO_TARGET}");
    let mut metadata = nested_fixture::nested_doc();
    metadata.about.clone_from(&payload);
    metadata.synopsis = Some(payload.clone());
    metadata.sections.headings.name.clone_from(&payload);
    metadata.sections.headings.options.clone_from(&payload);
    if let Some(subcommand) = metadata.subcommands.first_mut() {
        // A macro argument is interpreted once more than body text; the page
        // must still show the literal name.
        subcommand.bin_name = Some(r"tool\fBtail".to_owned());
    }
    metadata.sections.links = vec![LocalizedLink {
        text: Some(payload.clone()),
        uri: format!("https://example.invalid/{payload}"),
    }];
    let config = RoffConfig {
        date: Some(payload.clone()),
        source: Some(payload.clone()),
        manual: Some(payload),
        ..RoffConfig::default()
    };
    generate_to_string(&metadata, &config)
}

/// Writes the page to `temp_dir` and returns the file's path.
fn write_page(temp_dir: &tempfile::TempDir, page: &str) -> Result<Utf8PathBuf, Box<dyn Error>> {
    let out_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|path| format!("non-UTF-8 temp path: {}", path.display()))?;
    let dir = Dir::open_ambient_dir(&out_dir, ambient_authority())?;
    let mut file = dir.open_with(
        "page.1",
        OpenOptions::new().write(true).create(true).truncate(true),
    )?;
    file.write_all(page.as_bytes())?;
    Ok(out_dir.join("page.1"))
}

#[test]
fn groff_does_not_execute_injected_requests() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let path = write_page(&temp_dir, &adversarial_page()).expect("write the man page");
    let mut command = Command::new("groff");
    command
        .args(["-Tutf8", "-man", "-ww"])
        .arg(path.as_std_path());
    let Some(output) =
        run_optional(command, FORMAT_DEADLINE, "groff man page formatting").expect("run groff")
    else {
        return;
    };

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "groff failed on the generated page: {stderr}"
    );
    // Warnings about line breaking are acceptable: the adversarial payloads
    // deliberately produce long unbreakable tokens. An error means the page
    // asks the processor to do something it cannot, which is what an injected
    // request would do.
    assert!(
        !stderr.contains("error:"),
        "groff reported a parse error: {stderr}"
    );
    assert!(
        !stderr.contains(INJECTED_TM_MARKER),
        "the .tm payload was executed as a request: {stderr}"
    );
    assert!(
        !stderr.contains(INJECTED_SO_TARGET),
        "the .so payload was executed as a request: {stderr}"
    );
    assert!(
        stdout.contains("fixture"),
        "the processor should have formatted the page: {stdout}"
    );
    assert!(
        stdout.contains(r"tool\fBtail"),
        "a command name's backslash should survive as a literal: {stdout}"
    );
}

#[test]
fn mandoc_parses_the_generated_page() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let path = write_page(&temp_dir, &adversarial_page()).expect("write the man page");
    let mut command = Command::new("mandoc");
    command.args(["-Tlint"]).arg(path.as_std_path());
    let Some(output) =
        run_optional(command, FORMAT_DEADLINE, "mandoc man page linting").expect("run mandoc")
    else {
        return;
    };

    let diagnostics = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // `mandoc` documents one exit value per highest message level, and `-Tlint`
    // implies `-W all`: 0 is clean, 1 is a base-system or style remark, and 2
    // is a warning. A parsing error is 3, an unsupported feature 4, a bad
    // argument 5, and an operating-system error 6, so accepting 0-2 tolerates
    // the remarks an adversarial page may legitimately raise without accepting
    // a page that failed to parse. Signal termination has no code at all.
    //
    // The status is also the execution evidence. `-Tlint` echoes macro
    // arguments in its diagnostics, and `mandoc` quotes the whole `.TH` date
    // when it cannot parse it, so the payload markers legitimately appear in a
    // warning; asserting their absence would fail a correctly escaped page.
    // An unescaped `.tm` line instead reaches `mandoc` as an unknown macro,
    // and an unescaped `.so` as a failed request, both of which are errors.
    let status = output.status.code();
    assert!(
        matches!(status, Some(0..=2)),
        "mandoc reported a parse error or processing failure, status {status:?}: {diagnostics}"
    );
}
