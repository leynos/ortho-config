//! Integration tests for Cargo external-subcommand dispatch.
//!
//! Besides the dispatch diagnostics themselves, these tests pin the three
//! normal-completion process statuses documented for the CLI: `0` for a
//! successful run including help and version, `1` for an ordinary runtime
//! failure, and `2` for Clap's argument or dispatch rejection. Expected
//! failures also assert the exact number, because "not success" would accept a
//! crash or a signal.

mod fixtures;

#[path = "cli_dispatch/root_type_injection.rs"]
mod root_type_injection;

use insta::assert_snapshot;
use rstest::rstest;
use std::error::Error;
use std::process::{Command, Output};
use std::time::Duration;

/// Process status for a completed run, including `--help` and `--version`.
const EXIT_SUCCESS: i32 = 0;

/// Process status for an ordinary runtime failure.
const EXIT_FAILURE: i32 = 1;

/// Process status for Clap's argument and dispatch rejection.
const EXIT_USAGE: i32 = 2;

/// Budget for one spawned `cargo-orthohelp` invocation.
///
/// The nextest base tier terminates a test after ten 60-second periods, so a
/// command that hangs is reported here first, with its own description, rather
/// than as an unexplained test timeout.
const SUBPROCESS_DEADLINE: Duration = Duration::from_secs(480);

fn run_direct(args: &[&str]) -> Result<Output, Box<dyn Error + Send + Sync>> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(exe.as_str());
    command
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .args(args);
    Ok(test_helpers::process::run_with_deadline(
        command,
        SUBPROCESS_DEADLINE,
        "cargo-orthohelp direct invocation",
    )?)
}

fn run_cargo_dispatch(args: &[&str]) -> Result<Output, Box<dyn Error + Send + Sync>> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    let bin_dir = exe
        .parent()
        .ok_or_else(|| format!("cargo-orthohelp binary parent missing for {exe}"))?;
    let path = std::env::var_os("PATH")
        .ok_or_else(|| format!("PATH should be set to run Cargo dispatch for {exe}"))?;
    let paths =
        std::iter::once(bin_dir.as_std_path().to_path_buf()).chain(std::env::split_paths(&path));
    let joined_path = std::env::join_paths(paths)?;

    let mut command = Command::new("cargo");
    command
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .env("PATH", joined_path)
        // Dispatch only finds and runs the subcommand binary; it builds
        // nothing, and the offline setting states that for Concordat NC-007.
        .env("CARGO_NET_OFFLINE", "true")
        .arg("orthohelp")
        .args(args);
    Ok(test_helpers::process::run_with_deadline(
        command,
        SUBPROCESS_DEADLINE,
        "cargo-dispatched orthohelp invocation",
    )?)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn usage_lines(output: &Output) -> String {
    stdout(output)
        .lines()
        .filter(|line| line.starts_with("Usage:"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[rstest]
#[case::direct_top_level_help(
    "direct_top_level_help",
    &["--help"],
    false,
    "top-level direct help should succeed"
)]
#[case::direct_subcommand_help(
    "direct_subcommand_help",
    &["orthohelp", "--help"],
    false,
    "direct subcommand help should succeed"
)]
#[case::cargo_dispatch_help(
    "cargo_dispatch_help",
    &["--help"],
    true,
    "Cargo-dispatched help should succeed"
)]
fn help_output_matches_snapshots(
    #[case] snapshot_name: &str,
    #[case] args: &[&str],
    #[case] is_cargo_dispatch: bool,
    #[case] success_message: &str,
) {
    let output = if is_cargo_dispatch {
        run_cargo_dispatch(args).expect("Cargo-dispatched orthohelp should run")
    } else {
        run_direct(args).expect("direct orthohelp invocation should run")
    };

    assert_eq!(
        output.status.code(),
        Some(EXIT_SUCCESS),
        "{success_message}: {}",
        stderr(&output)
    );
    assert_snapshot!(snapshot_name, usage_lines(&output));
}

/// `--version` completes successfully at both command levels.
///
/// The documented status contract puts version alongside help in the success
/// class, and the banner must report the version this binary was built from.
/// The single-line assertion is what catches an extra stdout line, which a
/// status check alone would accept.
#[rstest]
#[case::direct_top_level_version(&["--version"], "direct --version should succeed")]
#[case::direct_subcommand_version(&["orthohelp", "--version"], "direct subcommand --version should succeed")]
fn version_reports_success_and_the_built_version(
    #[case] args: &[&str],
    #[case] success_message: &str,
) {
    let output = run_direct(args).expect("direct orthohelp invocation should run");

    assert_eq!(
        output.status.code(),
        Some(EXIT_SUCCESS),
        "{success_message}: {}",
        stderr(&output)
    );
    let banner = stdout(&output);
    assert_eq!(
        banner.lines().count(),
        1,
        "the version banner must stay one line, got:\n{banner}"
    );
    assert!(
        banner.contains(env!("CARGO_PKG_VERSION")),
        "the banner must name the built version {}: {banner}",
        env!("CARGO_PKG_VERSION")
    );
}

/// What a failing dispatch case must report.
///
/// The fields travel together because they describe one failure class: its
/// diagnostic, the usage hint it may add, and the status its class documents.
struct FailureExpectation {
    /// Failure message used in assertion diagnostics.
    message: &'static str,
    /// Text the diagnostic must contain.
    error: &'static str,
    /// Usage line the diagnostic must add, when the class has one.
    usage: Option<&'static str>,
    /// Process status the failure class documents.
    status: i32,
}

#[rstest]
#[case::direct_without_subcommand(
    &["--format", "ir"],
    FailureExpectation {
        message: "direct invocation without subcommand should fail",
        error: "unexpected argument '--format'",
        usage: Some("Usage: cargo <COMMAND>"),
        status: EXIT_USAGE,
    }
)]
#[case::unknown_subcommand(
    &["unknown"],
    FailureExpectation {
        message: "unknown subcommand should fail",
        error: "unrecognized subcommand 'unknown'",
        usage: Some("Usage: cargo <COMMAND>"),
        status: EXIT_USAGE,
    }
)]
#[case::invalid_subcommand_option(
    &["orthohelp", "--man-section", "9"],
    FailureExpectation {
        message: "invalid man section should fail",
        error: "invalid value '9'",
        usage: None,
        status: EXIT_USAGE,
    }
)]
// The root type is validated after the arguments have parsed, so a rejected
// value is an ordinary runtime failure rather than a usage error.
#[case::injection_shaped_root_type(
    &[
        "orthohelp",
        "--package",
        "orthohelp_fixture",
        "--root-type",
        concat!(
            "orthohelp_fixture::SimpleFixtureConfig as OrthoConfigDocs>::get_doc_metadata(); ",
            "std::process::exit(1); let _ = <orthohelp_fixture::SimpleFixtureConfig",
        ),
    ],
    FailureExpectation {
        message: "injection-shaped root type should fail",
        error: "InvalidRootType",
        usage: None,
        status: EXIT_FAILURE,
    }
)]
fn command_failure_cases(#[case] args: &[&str], #[case] expected: FailureExpectation) {
    let output = run_direct(args).expect("direct orthohelp invocation should run");

    assert_eq!(
        output.status.code(),
        Some(expected.status),
        "{}: expected the documented status for its failure class",
        expected.message
    );
    let stderr = stderr(&output);
    assert!(
        stderr.contains(expected.error),
        "unexpected error output:\n{stderr}"
    );
    if let Some(usage_text) = expected.usage {
        assert!(
            stderr.contains(usage_text),
            "unexpected error usage:\n{stderr}"
        );
    }
}

#[test]
fn direct_invocation_without_subcommand_includes_cargo_hint() {
    let output = run_direct(&["--format", "ir"]).expect("direct orthohelp invocation should run");

    assert_eq!(
        output.status.code(),
        Some(EXIT_USAGE),
        "direct invocation should be rejected as a usage error"
    );
    let stderr = stderr(&output);
    assert!(
        stderr.contains(
            "note: invoke this tool via `cargo orthohelp` or as `cargo-orthohelp orthohelp [OPTIONS]`"
        ),
        "unexpected error output:\n{stderr}"
    );
}

/// A runtime failure keeps status 1, distinct from Clap's usage status.
///
/// An unknown package is rejected while selecting the package, after Clap has
/// accepted the arguments, so it exercises the ordinary failure class rather
/// than the usage class. This is the assertion that would fail if the tool
/// collapsed every failure onto one non-zero number.
#[test]
fn missing_package_reports_failure_status() {
    let output = run_direct(&["orthohelp", "--package", "definitely-not-in-this-workspace"])
        .expect("direct orthohelp invocation should run");

    assert_eq!(
        output.status.code(),
        Some(EXIT_FAILURE),
        "a runtime failure should exit {EXIT_FAILURE}, stderr:\n{}",
        stderr(&output)
    );
    let diagnostics = stderr(&output);
    assert!(
        diagnostics.contains("PackageNotFound")
            && diagnostics.contains("definitely-not-in-this-workspace"),
        "the failure should name the missing package, stderr:\n{diagnostics}"
    );
}

#[rstest]
#[case::unknown_extra_argument(
    &["unknown"],
    "Cargo-dispatched unknown argument should fail",
    "unexpected argument 'unknown'",
    Some("Usage: cargo orthohelp [OPTIONS]")
)]
#[case::invalid_subcommand_option(
    &["--man-section", "9"],
    "Cargo-dispatched invalid man section should fail",
    "invalid value '9'",
    None
)]
fn cargo_dispatch_failure_cases(
    #[case] args: &[&str],
    #[case] failure_message: &str,
    #[case] expected_error: &str,
    #[case] expected_usage: Option<&str>,
) {
    let output = run_cargo_dispatch(args).expect("Cargo-dispatched orthohelp should run");

    assert_eq!(
        output.status.code(),
        Some(EXIT_USAGE),
        "{failure_message}: expected Clap's usage status"
    );
    let stderr = stderr(&output);
    assert!(
        stderr.contains(expected_error),
        "unexpected error output:\n{stderr}"
    );
    if let Some(usage_text) = expected_usage {
        assert!(
            stderr.contains(usage_text),
            "unexpected error usage:\n{stderr}"
        );
    }
}
