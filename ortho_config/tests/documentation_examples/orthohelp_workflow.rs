//! Compile, snapshot, and supervise the documented Cargo subcommand workflow.

use anyhow::{Context, Result};
use cap_std::{ambient_authority, fs::Dir};
use std::path::Path;
use std::process::Output;
#[path = "scenario.rs"]
mod scenario;

use crate::{
    build_state::BuildState,
    cargo_runner,
    process_runner::{self, Operation, Policy},
};

/// Snapshot Cargo's supplied CLI executable, then run
/// `cargo orthohelp <args> --out-dir <destination>` through Cargo's
/// external-subcommand dispatch and return its captured output.
///
/// The integration target belongs to cargo-orthohelp, so Cargo has already
/// built this executable before tests begin. Never rebuild a parent-owned
/// executable while another test may be running it (notably on Windows). The
/// caller judges the exit status and streams.
pub(super) fn run_orthohelp(
    repository: &Path,
    executable: &Path,
    args: &[&str],
    destination: &Path,
) -> Result<Output> {
    let deadline = scenario::deadline();
    let state = repository.join("target/documentation-workspaces");
    Dir::create_ambient_dir_all(&state, ambient_authority())?;
    let owner = tempfile::Builder::new()
        .prefix("orthohelp-")
        .tempdir_in(state)?;
    let private_bin = owner.path().join("executables");
    snapshot_cli(executable, &private_bin)?;
    BuildState::with_owned_state(repository, deadline, |_cache, target, remaining| {
        // The command invokes Cargo for metadata and the generated bridge, so
        // this is build infrastructure rather than a sanitized example binary.
        let mut command = cargo_runner::prepare_cargo_command_for_target_before(
            repository,
            owner.path(),
            target,
            deadline,
        )?;
        let inherited_path = std::env::var_os("PATH");
        let path = cargo_runner::cargo_dispatch_path(&private_bin, inherited_path.as_deref())?;
        command
            .env("CARGO_NET_OFFLINE", "true")
            .env("PATH", path)
            .arg("orthohelp")
            .args(args)
            .arg("--out-dir")
            .arg(destination);
        process_runner::run_with_policy(
            &mut command,
            Operation("run documented cargo-orthohelp flow"),
            Policy::build().bounded_by(scenario::remaining(deadline)?.min(remaining)),
        )
        .map_err(|error| {
            if process_runner::deadline_exceeded(&error) {
                error.context("documented cargo-orthohelp build deadline expired")
            } else if process_runner::cleanup_incomplete(&error) {
                error.context("documented cargo-orthohelp cleanup was not confirmed")
            } else {
                error.context("documented cargo-orthohelp subprocess failed")
            }
        })
    })
}

/// Copy a Cargo-owned executable without rebuilding its shared output path.
fn snapshot_cli(executable: &Path, private_bin: &Path) -> Result<()> {
    let source = Dir::open_ambient_dir(
        executable
            .parent()
            .context("Cargo executable has no directory")?,
        ambient_authority(),
    )?;
    let binary_name = executable
        .file_name()
        .context("Cargo executable has no filename")?;
    Dir::create_ambient_dir_all(private_bin, ambient_authority())?;
    let snapshot = Dir::open_ambient_dir(private_bin, ambient_authority())?;
    source.copy(binary_name, &snapshot, binary_name)?;
    Ok(())
}
