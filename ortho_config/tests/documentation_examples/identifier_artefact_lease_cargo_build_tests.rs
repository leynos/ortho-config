//! A real Cargo build keeps the lease after its helper dies.
//!
//! The sibling termination regression drives a synthetic writer, which is
//! deterministic but does not exercise the process tree the fixture actually
//! starts. This module drives a real `cargo build` instead. The helper leases
//! the fixture target and hands Cargo the inherited lease as its standard
//! input; the probe project's build script announces itself and then holds the
//! build open. The parent kills the helper, and the target must stay refused
//! while Cargo is still running, then become acquirable once the recorded tree
//! is terminated.
//!
//! Cargo gives its own children — build scripts and rustc — a null standard
//! input, so the lease reaches Cargo and stops there. The refusal proves the
//! handle the real build received; the termination proves the other half.
//!
//! On Unix the helper leads its own process group and Cargo another, so
//! [`Descendants`] records and terminates the tree that survives the kill. On
//! Windows the helper's Job Object owns that tree, and `process-wrap`
//! terminates the whole job when the helper is killed.

use super::identifier_artefact_e2e_support::acquire;
use super::identifier_artefact_e2e_support::locate_fixture;
#[cfg(unix)]
use super::identifier_artefact_e2e_support::{Descendants, snapshot_descendants};
#[cfg(unix)]
use super::identifier_artefact_lease_support::REFUSAL_WINDOW;
use super::identifier_artefact_lease_support::{
    HELPER_LEASE_WAIT, HELPER_STARTUP, HELPER_WORK_BOUND, Helper, REPLACEMENT_WINDOW,
    announced_target, required_scratch, required_scratch_path, wait_for_marker,
};
use super::{cargo_environment, process_runner, scenario};
#[cfg(unix)]
use anyhow::ensure;
use anyhow::{Context, Result};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use serial_test::serial;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use tempfile::TempDir;

/// The qualified name of the ignored helper this module drives.
const NESTED_CARGO_HELPER: &str =
    "identifier_artefact_lease_cargo_build_tests::nested_cargo_helper";

/// The directory, inside the scratch space, holding the probe Cargo project.
const PROBE_PROJECT: &str = "lease-probe";

/// The environment variable naming the marker the probe build script writes.
const PROBE_MARKER_VARIABLE: &str = "LEASE_PROBE_MARKER";

/// The marker the probe build script writes once it is running.
///
/// It lives in the scratch directory, beside the helper's own markers, so the
/// parent observes the build through the same capability handle it already
/// holds.
const PROBE_MARKER: &str = "build_started";

/// How long the probe build script holds the build open.
///
/// The parent kills the helper within seconds of the marker, so this only has
/// to outlast the parent's own waits; it is not a deadline the test relies on.
const PROBE_BUILD_HOLD_SECS: u64 = 60;

/// A dependency-free Cargo project whose build script holds the build open.
const PROBE_MANIFEST: &str = concat!(
    "[package]\n",
    "name = \"lease_probe\"\n",
    "version = \"0.1.0\"\n",
    "edition = \"2021\"\n",
    "build = \"build.rs\"\n",
    "\n",
    "[workspace]\n",
);

/// Lease the target, then run a real Cargo build beneath it until killed.
///
/// Ignored so an ordinary run never starts it; the test below selects it by
/// exact name. Cargo is the direct child that receives the inherited lease, so
/// it keeps the target held for as long as it lives, even though it gives its
/// own build script and rustc a null standard input. The probe build script
/// writes a marker and then holds the build open, so the parent can interrupt
/// this process while Cargo is still running.
#[test]
#[ignore = "driven as a child process by a_real_cargo_build_keeps_the_lease_after_its_helper_dies"]
fn nested_cargo_helper() -> Result<()> {
    let scratch = required_scratch()?;
    let scratch_path = required_scratch_path()?;
    // One envelope covers discovery, the lease and the build, and stays inside
    // the parent's bound so this process's supervised Cargo reaches its own
    // deadline before the parent stops waiting.
    let deadline = scenario::deadline().min(Instant::now() + HELPER_WORK_BOUND);
    let location = locate_fixture(deadline)?;
    let target = location.target_directory().to_path_buf();
    scratch
        .write("target.txt", target.display().to_string())
        .context("announce the leased target")?;
    // Capped below the parent's marker window so a contended target fails here
    // with the lease's own diagnostic instead of being killed mid-wait.
    let cargo = location.lease_within(deadline, HELPER_LEASE_WAIT, None)?;
    write_probe_project(&scratch)?;
    scratch
        .write("lease_held", b"held")
        .context("announce the held lease")?;
    let mut command = probe_build_command(&scratch_path);
    command.env(PROBE_MARKER_VARIABLE, scratch_path.join(PROBE_MARKER));
    // The runner terminates Cargo at this bound. Nothing is asserted here: the
    // parent kills this process first, which is the interruption under test.
    let _run = process_runner::run_with_policy_with_stdin(
        &mut command,
        process_runner::Operation("run a real Cargo build beneath the helper"),
        process_runner::Policy::build().bounded_by(HELPER_WORK_BOUND),
        cargo.inherited_lease()?,
    );
    Ok(())
}

/// A killed helper's real Cargo build keeps the lease after the helper dies.
///
/// This is the ordering the fixture build itself relies on. Cargo is the only
/// process that received the inherited handle, so it holds the target while
/// its build script runs; the parent records and terminates the tree to
/// release it.
#[test]
#[serial]
fn a_real_cargo_build_keeps_the_lease_after_its_helper_dies() -> Result<()> {
    let scratch = TempDir::new().context("create the helper scratch directory")?;
    let scratch_directory = Dir::open_ambient_dir(scratch.path(), ambient_authority())
        .context("open the helper scratch directory")?;
    let mut helper = Helper::spawn_for(NESTED_CARGO_HELPER, scratch.path())?;
    wait_for_marker(
        &mut helper,
        &scratch_directory,
        "lease_held",
        HELPER_STARTUP,
    )?;
    let target = announced_target(&scratch_directory)?;
    // Waiting for the build script's own marker proves the build is running,
    // rather than merely that the helper has announced its lease.
    wait_for_marker(
        &mut helper,
        &scratch_directory,
        PROBE_MARKER,
        HELPER_STARTUP,
    )?;

    #[cfg(unix)]
    let descendants = snapshot_descendants(helper.pid())?;
    helper.kill_alone()?;
    let observation = BuildObservation {
        target,
        #[cfg(unix)]
        descendants,
    };
    observation.assert_real_build_was_removed()
}

/// Write the dependency-free probe project the real build compiles.
fn write_probe_project(scratch: &Dir) -> Result<()> {
    scratch
        .create_dir_all(PROBE_PROJECT)
        .context("create the probe Cargo project")?;
    let project = scratch
        .open_dir(PROBE_PROJECT)
        .context("open the probe Cargo project")?;
    project
        .write("Cargo.toml", PROBE_MANIFEST)
        .context("write the probe manifest")?;
    project
        .write("build.rs", probe_build_script())
        .context("write the probe build script")?;
    project
        .create_dir_all("src")
        .context("create the probe source directory")?;
    project
        .write("src/lib.rs", "")
        .context("write the probe library")?;
    Ok(())
}

/// The probe build script: announce the build, then hold it open.
///
/// The marker is written before the hold, so the parent never kills the helper
/// before the build has started. The hold is what keeps Cargo and its build
/// script alive long enough for the parent to interrupt them.
fn probe_build_script() -> String {
    format!(
        concat!(
            "fn main() {{\n",
            "    let marker = std::env::var(\"{marker}\")\n",
            "        .expect(\"the parent should name the marker path\");\n",
            "    std::fs::write(&marker, b\"building\").expect(\"write the build marker\");\n",
            "    std::thread::sleep(std::time::Duration::from_secs({hold}));\n",
            "}}\n",
        ),
        marker = PROBE_MARKER_VARIABLE,
        hold = PROBE_BUILD_HOLD_SECS,
    )
}

/// The `cargo build` command for the probe project.
fn probe_build_command(scratch_path: &Path) -> Command {
    let mut command = Command::new("cargo");
    cargo_environment::CargoEnvironment::build().apply(&mut command);
    command
        .args(["build", "--offline"])
        .current_dir(scratch_path.join(PROBE_PROJECT));
    command
}

/// What the parent still holds once the real-build helper has been killed.
struct BuildObservation {
    /// The leased target, for replacement acquisitions.
    target: PathBuf,
    /// The helper's descendants, recorded before it was killed.
    #[cfg(unix)]
    descendants: Descendants,
}

impl BuildObservation {
    /// Cargo must keep the lease until the recorded tree is terminated.
    ///
    /// Unlike the synthetic writer, the orphan here is Cargo itself: it is the
    /// process that received the inherited handle, and the build script beneath
    /// it never did. The refusal therefore proves the real inherited lease, not
    /// a writer the test started by hand.
    #[cfg(unix)]
    fn assert_real_build_was_removed(&self) -> Result<()> {
        ensure!(
            !self.descendants.is_empty(),
            "the snapshot should record the Cargo build the helper started",
        );
        let _refusal = acquire(&self.target, Instant::now() + REFUSAL_WINDOW)
            .err()
            .context(
                "a replacement acquired the target while the orphaned Cargo build still held \
                 its inherited lease",
            )?;
        self.descendants.terminate()?;
        let _replacement = acquire(&self.target, Instant::now() + REPLACEMENT_WINDOW).context(
            "the target should be acquirable once the orphaned Cargo build is terminated",
        )?;
        Ok(())
    }

    /// Terminating the helper's job must kill Cargo and free the target.
    #[cfg(not(unix))]
    fn assert_real_build_was_removed(&self) -> Result<()> {
        // `process-wrap`'s Job Object terminates the whole job when the child
        // is killed, so Cargo died with the helper and released the lease.
        let _replacement = acquire(&self.target, Instant::now() + REPLACEMENT_WINDOW)
            .context("terminating the helper's job object should release the inherited lease")?;
        Ok(())
    }
}
