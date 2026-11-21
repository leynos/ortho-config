//! Integration tests for Cargo external-subcommand dispatch.
//!
//! Besides the dispatch diagnostics themselves, these tests pin the three
//! normal-completion process statuses documented for the CLI: `0` for a
//! successful run including help and version, `1` for an ordinary runtime
//! failure, and `2` for Clap's argument or dispatch rejection. Expected
//! failures also assert the exact number, because "not success" would accept a
//! crash or a signal.

mod fixtures;

use camino::Utf8Path;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
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
        concat!(
            "orthohelp_fixture::SimpleFixtureConfig as OrthoConfigDocs>::get_doc_metadata(); ",
            "std::process::exit(1); let _ = <orthohelp_fixture::SimpleFixtureConfig",
        ),
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

/// Writes a complete, buildable package for the metadata injection payload.
///
/// The fixture derives every trait the bridge requires and defaults its field,
/// so the payload would compile and write its marker if validation were
/// bypassed.
fn write_injection_fixture(
    dir: &Dir,
    ortho_config_path: &Utf8Path,
    root_type: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    dir.create_dir("src")?;
    dir.write(
        "src/lib.rs",
        concat!(
            "use clap::Parser;\n",
            "use ortho_config::OrthoConfig;\n",
            "use serde::{Deserialize, Serialize};\n",
            "\n",
            "#[derive(Debug, Clone, PartialEq, Eq, Parser, Deserialize, Serialize, OrthoConfig)]\n",
            "#[ortho_config(prefix = \"ROOT_TYPE_INJECTION\")]\n",
            "pub struct Config {\n",
            "    /// Value used only to satisfy the derive's named-field rule.\n",
            "    #[ortho_config(default = String::from(\"value\"))]\n",
            "    pub value: String,\n",
            "}\n",
        ),
    )?;
    dir.write(
        "Cargo.toml",
        format!(
            concat!(
                "[package]\n",
                "name = \"root_type_injection_fixture\"\n",
                "version = \"0.1.0\"\n",
                "edition = \"2024\"\n",
                "\n",
                "[dependencies]\n",
                "ortho_config = {{ path = {ortho_config:?} }}\n",
                "serde = {{ version = \"1\", features = [\"derive\"] }}\n",
                "clap = {{ version = \"4\", features = [\"derive\"] }}\n",
                "\n",
                "[package.metadata.ortho_config]\n",
                "root_type = {root_type:?}\n",
            ),
            ortho_config = ortho_config_path.as_str(),
            root_type = root_type,
        ),
    )?;
    Ok(())
}

/// Asserts the generator rejected the root type with the semantic diagnostic.
fn assert_root_type_rejected(
    output: &Output,
    failure_message: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    if output.status.success() {
        return Err(failure_message.into());
    }
    let stderr = stderr(output);
    if !stderr.contains("InvalidRootType") {
        return Err(format!("unexpected diagnostic:\n{stderr}").into());
    }
    Ok(())
}

/// Asserts the bridge never ran by checking the payload's marker is absent.
fn assert_marker_absent(dir: &Dir) -> Result<(), Box<dyn Error + Send + Sync>> {
    if dir.exists("injected.marker") {
        return Err("the bridge executed the injected payload".into());
    }
    Ok(())
}

/// The payload would be executable if validation were bypassed: it names a real
/// root type from the fixture and writes a marker file as a side effect.
#[test]
fn cli_root_type_injection_is_rejected_before_the_bridge_runs()
-> Result<(), Box<dyn Error + Send + Sync>> {
    let marker_dir = tempfile::tempdir()?;
    let marker_path = marker_dir.path().join("injected.marker");
    let payload = format!(
        concat!(
            "orthohelp_fixture::SimpleFixtureConfig as OrthoConfigDocs>::",
            "get_doc_metadata(); std::fs::write({marker:?}, b\"injected\")",
            ".expect(\"marker write\"); let _ = <orthohelp_fixture::SimpleFixtureConfig",
        ),
        marker = marker_path,
    );

    let output = run_direct(&[
        "orthohelp",
        "--package",
        "orthohelp_fixture",
        "--root-type",
        &payload,
    ])?;
    assert_root_type_rejected(&output, "injection-shaped root type should fail")?;

    let marker_dir_path = Utf8Path::from_path(marker_dir.path())
        .ok_or("temporary marker path should be valid UTF-8")?;
    let dir = Dir::open_ambient_dir(marker_dir_path, ambient_authority())?;
    assert_marker_absent(&dir)
}

#[test]
fn metadata_root_type_injection_is_rejected() -> Result<(), Box<dyn Error + Send + Sync>> {
    let package_dir = tempfile::tempdir()?;
    let package_path = Utf8Path::from_path(package_dir.path())
        .ok_or("temporary package path should be valid UTF-8")?;
    let dir = Dir::open_ambient_dir(package_path, ambient_authority())?;
    let marker_path = package_dir.path().join("injected.marker");
    let root_type = format!(
        concat!(
            "root_type_injection_fixture::Config as OrthoConfigDocs>::get_doc_metadata(); ",
            "std::fs::write({marker:?}, b\"injected\").expect(\"marker write\"); ",
            "let _ = <root_type_injection_fixture::Config",
        ),
        marker = marker_path,
    );
    let ortho_config_path = fixtures::workspace_root()?.join("ortho_config");
    write_injection_fixture(&dir, &ortho_config_path, &root_type)?;

    let exe = fixtures::cargo_orthohelp_exe()?;
    let mut command = Command::new(exe.as_str());
    command.current_dir(package_dir.path()).args([
        "orthohelp",
        "--format",
        "agent-context",
        "--out-dir",
        "out",
    ]);
    let output = test_helpers::process::run_with_deadline(
        command,
        SUBPROCESS_DEADLINE,
        "cargo-orthohelp metadata injection probe",
    )?;

    assert_root_type_rejected(&output, "metadata root type injection should fail")?;
    assert_marker_absent(&dir)
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
