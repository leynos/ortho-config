//! Build, clean and read the identifier-artefact fixture.
//!
//! Every invocation here runs beneath the whole-test lease held by
//! [`FixtureCargo`] and inherits a duplicate of its handle, so the target stays
//! leased for as long as a compiler started by the invocation can still write
//! to it. That is what makes a `cargo clean -p orthohelp_fixture` from another
//! test process wait, rather than delete an artefact this build is writing.
//!
//! The functions are split from discovery and leasing so the support module
//! stays a readable size; they form one phase of the fixture's life: what
//! happens once the target is exclusively ours.

use super::cargo_messages::{BuildCounts, CargoMessages, lock_cargo_messages};
use super::telemetry::PhaseRecord;
use super::{FixtureCargo, cargo_environment, process_runner};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

/// What one fixture build produced and what it reused.
pub(crate) struct FixtureBuild {
    /// The build script's output directory.
    out_dir: PathBuf,
    /// Fresh and rebuilt unit counts Cargo reported for this build.
    counts: BuildCounts,
}

impl FixtureBuild {
    /// The build script's output directory, where the artefact is emitted.
    pub(crate) fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    /// Which Cargo units were reused and which were recompiled.
    pub(crate) const fn counts(&self) -> BuildCounts {
        self.counts
    }
}

/// Build the fixture with optional artefact emission and report what it reused.
pub(crate) fn build_fixture(
    emit: bool,
    cargo: &FixtureCargo,
    previous_out_dir: Option<&Path>,
) -> Result<FixtureBuild> {
    let mut command = Command::new("cargo");
    cargo_environment::CargoEnvironment::build().apply(&mut command);
    command
        .args([
            "build",
            "-p",
            "orthohelp_fixture",
            "--message-format=json-render-diagnostics",
        ])
        .arg("--target-dir")
        .arg(&cargo.target_directory)
        .current_dir(&cargo.repository)
        .env_remove("ORTHO_CONFIG_EMIT_IDENTIFIERS");
    if emit {
        command.env("ORTHO_CONFIG_EMIT_IDENTIFIERS", "1");
    }
    let messages = Arc::new(Mutex::new(CargoMessages::for_package(
        cargo.package_id.clone(),
    )));
    let observed_messages = Arc::clone(&messages);
    // Every compiler Cargo starts inherits the lease handle this process hands
    // the nested build, so a replacement cannot clean the target mid-build or
    // after this process is interrupted while Cargo is still running.
    let output = process_runner::run_with_hooks(
        &mut command,
        process_runner::Operation("build identifier artefact fixture"),
        cargo.bounded_policy(
            process_runner::Policy::build(),
            "build identifier artefact fixture",
        )?,
        process_runner::RunHooks {
            stdin: Some(cargo.inherited_lease()?),
            observer: move |chunk: &[u8]| lock_cargo_messages(&observed_messages).observe(chunk),
            poll: process_runner::poll_child,
            kill: process_runner::kill_child,
        },
    )
    .map_err(|error| retain_runner_error(error, "build identifier artefact fixture"))?;
    ensure!(
        output.status.success(),
        "fixture build should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut report = lock_cargo_messages(&messages);
    let out_dir = report
        .finish()?
        .or_else(|| previous_out_dir.map(Path::to_path_buf))
        .context("Cargo output omitted the fixture build-script directory")?;
    let counts = report.counts();
    if let Some(telemetry) = cargo.telemetry() {
        telemetry.record(PhaseRecord {
            emit,
            counts,
            stderr: &output.stderr,
        })?;
    }
    Ok(FixtureBuild { out_dir, counts })
}

/// Remove only the fixture's build output in its private target directory.
pub(crate) fn clean_fixture(cargo: &FixtureCargo) -> Result<()> {
    let mut command = Command::new("cargo");
    cargo_environment::CargoEnvironment::build().apply(&mut command);
    command
        .args(["clean", "-p", "orthohelp_fixture", "--target-dir"])
        .arg(&cargo.target_directory)
        .current_dir(&cargo.repository);
    let output = process_runner::run_with_policy_with_stdin(
        &mut command,
        process_runner::Operation("clean identifier artefact fixture outputs"),
        cargo.bounded_policy(
            process_runner::Policy::build(),
            "clean identifier artefact fixture outputs",
        )?,
        cargo.inherited_lease()?,
    )
    .map_err(|error| retain_runner_error(error, "clean identifier artefact fixture outputs"))?;
    ensure!(
        output.status.success(),
        "fixture package clean should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

/// Return the artefact path, if the fixture build emitted one.
pub(crate) fn artefact_path(out_dir: &Path) -> Result<Option<PathBuf>> {
    let directory = match Dir::open_ambient_dir(out_dir, ambient_authority()) {
        Ok(directory) => directory,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).context("open fixture build-script output directory");
        }
    };
    let relative_path = Path::new("ortho-config/cli-identifiers.json");
    Ok(directory
        .exists(relative_path)
        .then(|| out_dir.join(relative_path)))
}

/// Read the emitted identifier artefact, failing when it is absent.
pub(crate) fn artefact(out_dir: &Path) -> Result<String> {
    let directory = Dir::open_ambient_dir(out_dir, ambient_authority())
        .context("open fixture build-script output directory")?;
    directory
        .read_to_string("ortho-config/cli-identifiers.json")
        .context("read identifier artefact")
}

/// Add the operation to a runner error without losing its classification.
fn retain_runner_error(error: anyhow::Error, operation: &str) -> anyhow::Error {
    if process_runner::deadline_exceeded(&error) {
        error.context(format!("{operation} exceeded its overall deadline"))
    } else if process_runner::cleanup_incomplete(&error) {
        error.context(format!("{operation} cleanup was not confirmed"))
    } else {
        error.context(format!("{operation} subprocess failed"))
    }
}
