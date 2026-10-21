//! Verify subprocess diagnostics expose safe environment presence only.

use super::super::RunContext;
use anyhow::{Result, ensure};
use std::io;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn safe_environment_presence_does_not_expose_secret_values() -> Result<()> {
    let mut command = Command::new("cargo");
    command
        .env("RUSTC_WRAPPER", "/private/toolchain/rustc-wrapper")
        .env("CARGO_HOME", "/private/cache")
        .env("API_TOKEN", "API_TOKEN_DIAGNOSTIC_SENTINEL")
        .env("AWS_SECRET_ACCESS_KEY", "AWS_SECRET_DIAGNOSTIC_SENTINEL");
    let started = Instant::now();
    let context = RunContext {
        operation: "redact command environment".to_owned(),
        workspace: "private workspace".to_owned(),
        environment: super::environment_evidence(&command),
        started,
        deadline: started + Duration::from_secs(1),
        policy: super::super::Policy::new(
            "execution",
            Duration::from_secs(1),
            Duration::from_millis(50),
            1024,
        ),
        process_id: 1,
    };

    let diagnostic = super::format_failure_diagnostic(&context, &[], &[])?;

    ensure!(
        diagnostic.contains("RUSTC_WRAPPER=command-set"),
        "diagnostic omitted safe wrapper presence metadata: {diagnostic}"
    );
    ensure!(
        diagnostic.contains("CARGO_HOME=command-set"),
        "diagnostic omitted safe cache presence metadata: {diagnostic}"
    );
    ensure!(
        !diagnostic.contains("API_TOKEN_DIAGNOSTIC_SENTINEL")
            && !diagnostic.contains("AWS_SECRET_DIAGNOSTIC_SENTINEL")
            && !diagnostic.contains("/private/cache")
            && !diagnostic.contains("/private/toolchain/rustc-wrapper"),
        "diagnostic exposed a command environment value: {diagnostic}"
    );
    Ok(())
}

/// A failed working-directory query is named, not collapsed into a blank.
///
/// The queried directory is supplied rather than provoked: no portable
/// environmental change makes `std::env::current_dir` fail on demand.
#[test]
fn a_failed_working_directory_query_is_reported_explicitly() {
    let identity = super::inherited_workspace_identity(|| {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    });

    assert!(
        identity.starts_with("unavailable: "),
        "failed query was not named: {identity}"
    );
    assert!(
        identity.contains("permission denied"),
        "failed query omitted why it failed: {identity}"
    );
}

/// A command-local directory wins and never consults the ambient directory.
#[test]
fn a_command_directory_overrides_the_inherited_workspace() -> Result<()> {
    let mut command = Command::new("cargo");
    command.current_dir(PathBuf::from("/private/command/directory"));

    let identity = super::workspace_identity(&command);

    ensure!(identity == "/private/command/directory");
    Ok(())
}
