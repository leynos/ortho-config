//! Integration tests for Cargo external-subcommand dispatch.

mod fixtures;

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use insta::assert_snapshot;
use rstest::rstest;
use std::error::Error;
use std::process::{Command, Output};

fn run_direct(args: &[&str]) -> Result<Output, Box<dyn Error + Send + Sync>> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    Ok(Command::new(exe.as_str())
        .current_dir(fixtures::workspace_root()?.as_std_path())
        .args(args)
        .output()?)
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

    assert!(
        output.status.success(),
        "{success_message}: {}",
        stderr(&output)
    );
    assert_snapshot!(snapshot_name, usage_lines(&output));
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
#[case::injection_shaped_root_type(
    &[
        "orthohelp",
        "--package",
        "orthohelp_fixture",
        "--root-type",
        "Config as OrthoConfigDocs>::get_doc_metadata(); std::process::exit(1); let _ = <Config",
    ],
    "injection-shaped root type should fail",
    "InvalidRootType",
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

#[test]
fn direct_invocation_without_subcommand_includes_cargo_hint() {
    let output = run_direct(&["--format", "ir"]).expect("direct orthohelp invocation should run");

    assert!(!output.status.success(), "direct invocation should fail");
    let stderr = stderr(&output);
    assert!(
        stderr.contains(
            "note: invoke this tool via `cargo orthohelp` or as `cargo-orthohelp orthohelp [OPTIONS]`"
        ),
        "unexpected error output:\n{stderr}"
    );
}

#[test]
fn metadata_root_type_injection_is_rejected() -> Result<(), Box<dyn Error + Send + Sync>> {
    let package_dir = tempfile::tempdir()?;
    let package_path = Utf8Path::from_path(package_dir.path())
        .ok_or("temporary package path should be valid UTF-8")?;
    let dir = Dir::open_ambient_dir(package_path, ambient_authority())?;
    dir.create_dir("src")?;
    dir.write("src/lib.rs", "pub struct Config;\n")?;
    dir.write(
        "Cargo.toml",
        concat!(
            "[package]\n",
            "name = \"root_type_injection_fixture\"\n",
            "version = \"0.1.0\"\n",
            "edition = \"2024\"\n",
            "\n",
            "[package.metadata.ortho_config]\n",
            "root_type = \"Config as OrthoConfigDocs>::get_doc_metadata(); ",
            "std::process::exit(1); let _ = <Config\"\n",
        ),
    )?;

    let exe = fixtures::cargo_orthohelp_exe()?;
    let output = Command::new(exe.as_str())
        .current_dir(package_dir.path())
        .args(["orthohelp", "--format", "agent-context", "--out-dir", "out"])
        .output()?;

    if output.status.success() {
        return Err("metadata root type injection should fail".into());
    }
    let stderr = stderr(&output);
    if !stderr.contains("InvalidRootType") {
        return Err(format!("unexpected diagnostic:\n{stderr}").into());
    }
    Ok(())
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
