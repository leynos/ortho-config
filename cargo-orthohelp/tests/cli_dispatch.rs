//! Integration tests for Cargo external-subcommand dispatch.

mod fixtures;

use rstest::rstest;
use std::error::Error;
use std::process::{Command, Output};

fn run_direct(args: &[&str]) -> Result<Output, Box<dyn Error>> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    Ok(Command::new(exe.as_str())
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .args(args)
        .output()?)
}

fn run_cargo_dispatch(args: &[&str]) -> Result<Output, Box<dyn Error>> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    let bin_dir = exe
        .parent()
        .ok_or_else(|| format!("cargo-orthohelp binary parent missing for {exe}"))?;
    let path = std::env::var_os("PATH")
        .ok_or_else(|| format!("PATH should be set to run Cargo dispatch for {exe}"))?;
    let paths =
        std::iter::once(bin_dir.as_std_path().to_path_buf()).chain(std::env::split_paths(&path));
    let joined_path = std::env::join_paths(paths)?;

    Ok(Command::new("cargo")
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .env("PATH", joined_path)
        .arg("orthohelp")
        .args(args)
        .output()?)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[rstest]
#[case::direct(&["orthohelp", "--help"], false, "help should succeed")]
#[case::cargo_dispatch(&["--help"], true, "Cargo-dispatched help should succeed")]
fn help_output_uses_cargo_usage(
    #[case] args: &[&str],
    #[case] is_cargo_dispatch: bool,
    #[case] success_message: &str,
) {
    let output = if is_cargo_dispatch {
        run_cargo_dispatch(args).expect("Cargo-dispatched orthohelp should run")
    } else {
        run_direct(args).expect("direct orthohelp invocation should run")
    };

    assert!(
        output.status.success(),
        "{success_message}: {}",
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("Usage: cargo orthohelp [OPTIONS]"),
        "unexpected help output:\n{}",
        stdout(&output)
    );
}

#[rstest]
#[case::direct_without_subcommand(
    &["--format", "ir"],
    "direct invocation without subcommand should fail",
    "unexpected argument '--format'",
    Some("Usage: cargo <COMMAND>")
)]
#[case::unknown_subcommand(
    &["unknown"],
    "unknown subcommand should fail",
    "unrecognized subcommand 'unknown'",
    Some("Usage: cargo <COMMAND>")
)]
#[case::invalid_subcommand_option(
    &["orthohelp", "--man-section", "9"],
    "invalid man section should fail",
    "invalid value '9'",
    None
)]
fn command_failure_cases(
    #[case] args: &[&str],
    #[case] failure_message: &str,
    #[case] expected_error: &str,
    #[case] expected_usage: Option<&str>,
) {
    let output = run_direct(args).expect("direct orthohelp invocation should run");

    assert!(!output.status.success(), "{failure_message}");
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

    assert!(!output.status.success(), "{failure_message}");
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
