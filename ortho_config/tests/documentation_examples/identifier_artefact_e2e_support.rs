//! Cargo helpers owned by the identifier-artefact integration test.
//!
//! The fixture uses a private target nested under Cargo's configured target
//! directory so its clean operations cannot delete workspace test binaries.
//! Cargo still uses the default shared package cache.
//!
//! The private target is stable across runs, not a per-run temporary directory.
//! The phases that must force a rebuild start with [`clean_fixture`], so the
//! fixture package and its build script are rebuilt and report their output
//! directory; what the stable target preserves is the dependency build beneath
//! them. The warm phase deliberately does *not* clean: its contract is that a
//! warm build leaves the emitted artefact alone. A per-run target rebuilt
//! `ortho_config`, its macros, Clap and Serde from cold on every invocation, so
//! every phase paid a full dependency build; the fixed target makes only the
//! first phase pay it. That alone does not attribute an earlier envelope
//! exhaustion — the failing run discarded the child's output, this target
//! compiles with `--all-features` while the fixture uses its default features,
//! and both share Cargo's package cache — but it is the recurring cost the
//! fixed target removes, and the retained diagnostic tail now names a
//! compile-admission wait instead of leaving the failure silent. Only
//! `orthohelp_fixture` itself is cleaned between phases, which is what the
//! test's warm-build claim is about.
//!
//! A stable target is also shared state. nextest's `boundary_packaging` group
//! and the test's `#[serial]` attribute coordinate tests inside one runner, but
//! neither excludes a second test process — another `cargo test` run, a
//! `make test`, or a nextest invocation for another target — from interleaving
//! its own `cargo clean -p orthohelp_fixture` with this process's build or
//! artefact read. [`FixtureCargo`] therefore holds an exclusive lease file
//! inside the target for the whole test, from discovery until the fixture is
//! dropped; see [`fixture_cargo`].

#[path = "identifier_artefact_cargo_messages.rs"]
mod cargo_messages;
#[path = "identifier_artefact_lease.rs"]
mod lease;

use self::cargo_messages::{BuildCounts, CargoMessages, lock_cargo_messages};
use super::{cargo_environment, process_runner, scenario};
use anyhow::{Context, Result, ensure};
use cap_std::{
    ambient_authority,
    fs::{Dir, OpenOptions},
};
use serde_json::Value;
use std::io;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const CACHEDIR_TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n# For information about cache directory tags see https://bford.info/cachedir/\n";

/// Names the fixture's private Cargo target.
///
/// Nested under the workspace target so `cargo clean` of the workspace removes
/// it, and named so consecutive runs reuse the dependency build.
const TARGET_DIRECTORY_NAME: &str = "identifier-artefact-target";

/// Names the file to which each build phase appends its Cargo unit counts.
///
/// Used by cold-versus-warm evidence runs; unset in ordinary runs.
const TELEMETRY_VARIABLE: &str = "ORTHO_CONFIG_FIXTURE_TELEMETRY";

/// The fixture package and its private target, before the cross-process lease.
///
/// Discovery runs `cargo metadata`, which only reads the workspace and touches
/// no build output, so it is safe to perform while another test process holds
/// the lease. Splitting discovery from [`FixtureLocation::lease`] lets a
/// regression observe that a second process has reached the target and is
/// blocked on the lease, rather than merely slow to start.
pub(super) struct FixtureLocation {
    repository: PathBuf,
    target_directory: PathBuf,
    package_id: String,
}

impl FixtureLocation {
    /// Take the whole-test lease on the private target and build the fixture.
    ///
    /// `lock` blocks until every other leaseholder has released the
    /// target, so a concurrent test process cannot interleave its
    /// `cargo clean -p orthohelp_fixture` with this test's build or artefact
    /// read. [`serial_test`] and nextest's process group cannot provide this:
    /// they coordinate one runner's tests, and `#[serial]` is a thread lock
    /// inside one process.
    ///
    /// The scenario deadline is captured after the wait, so time spent queued
    /// behind another test process is not charged to this scenario's envelope.
    pub(super) fn lease(self) -> Result<FixtureCargo> {
        let lease = lease::acquire(&self.target_directory)?;
        Ok(FixtureCargo {
            repository: self.repository,
            target_directory: self.target_directory,
            package_id: self.package_id,
            deadline: scenario::deadline(),
            _lease: lease,
        })
    }
}

/// Describes the fixture's private Cargo target and scenario deadline.
///
/// Holding this value holds the target's cross-process lease; the field is
/// never read because the kernel does the work when the handle closes.
pub(super) struct FixtureCargo {
    repository: PathBuf,
    target_directory: PathBuf,
    package_id: String,
    deadline: Instant,
    _lease: lease::TargetLease,
}

impl FixtureCargo {
    fn bounded_policy(
        &self,
        policy: process_runner::Policy,
        operation: &str,
    ) -> Result<process_runner::Policy> {
        let remaining = scenario::remaining(self.deadline)
            .with_context(|| format!("{operation}: scenario deadline elapsed"))?;
        Ok(policy.bounded_by(remaining))
    }
}

/// Discover the fixture package and take the whole-test lease on its target.
///
/// The target directory is stable across runs; its dependencies stay warm while
/// each rebuilding phase's [`clean_fixture`] forces the fixture package itself
/// to rebuild. The returned value owns the lease file handle, so the target
/// stays reserved until the test drops it or ends.
pub(super) fn fixture_cargo() -> Result<FixtureCargo> {
    locate_fixture()?.lease()
}

/// Discover the fixture package and locate its private Cargo target.
///
/// This is the half of [`fixture_cargo`] that is safe to run concurrently with
/// another process's lease: it only reads workspace metadata and creates the
/// target directory, which is idempotent.
pub(super) fn locate_fixture() -> Result<FixtureLocation> {
    let deadline = scenario::deadline();
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("find fixture workspace")?
        .to_path_buf();
    let mut command = Command::new("cargo");
    cargo_environment::CargoEnvironment::metadata().apply(&mut command);
    command
        .args(["metadata", "--offline", "--no-deps", "--format-version=1"])
        .current_dir(&repository);
    let remaining =
        scenario::remaining(deadline).context("discover Cargo target for identifier artefact")?;
    let output = process_runner::run_with_policy(
        &mut command,
        process_runner::Operation("discover Cargo target for identifier artefact"),
        process_runner::Policy::discovery().bounded_by(remaining),
    )?;
    ensure!(
        output.status.success(),
        "Cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let metadata: Value =
        serde_json::from_slice(&output.stdout).context("parse Cargo fixture metadata")?;
    let workspace_target = metadata
        .get("target_directory")
        .and_then(Value::as_str)
        .context("Cargo metadata omitted its target directory")?;
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .context("Cargo metadata omitted its workspace packages")?;
    let package_id = packages
        .iter()
        .find(|package| package.get("name").and_then(Value::as_str) == Some("orthohelp_fixture"))
        .and_then(|package| package.get("id"))
        .and_then(Value::as_str)
        .context("Cargo metadata omitted orthohelp_fixture's package id")?;
    let target_directory = Path::new(workspace_target).join(TARGET_DIRECTORY_NAME);
    Dir::create_ambient_dir_all(&target_directory, ambient_authority())
        .context("create private Cargo target")?;
    Dir::open_ambient_dir(&target_directory, ambient_authority())?
        .write("CACHEDIR.TAG", CACHEDIR_TAG.as_bytes())
        .context("write Cargo cache-directory tag")?;
    Ok(FixtureLocation {
        repository,
        target_directory,
        package_id: package_id.to_owned(),
    })
}

/// What one fixture build produced and what it reused.
pub(super) struct FixtureBuild {
    /// The build script's output directory.
    out_dir: PathBuf,
    /// Fresh and rebuilt unit counts Cargo reported for this build.
    counts: BuildCounts,
}

impl FixtureBuild {
    /// The build script's output directory, where the artefact is emitted.
    pub(super) fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    /// Which Cargo units were reused and which were recompiled.
    pub(super) const fn counts(&self) -> BuildCounts {
        self.counts
    }
}

/// Build the fixture with optional artefact emission and report what it reused.
pub(super) fn build_fixture(
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
    let output = process_runner::run_with_observer(
        &mut command,
        process_runner::Operation("build identifier artefact fixture"),
        cargo.bounded_policy(
            process_runner::Policy::build(),
            "build identifier artefact fixture",
        )?,
        move |chunk| lock_cargo_messages(&observed_messages).observe(chunk),
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
    record_phase(emit, counts, &output.stderr)?;
    Ok(FixtureBuild { out_dir, counts })
}

/// Append one phase's build counts to the file named by the telemetry variable.
///
/// This exists so a cold-versus-warm evidence run can show, phase by phase,
/// which Cargo units were reused, when the phase ran, and whether the nested
/// Cargo waited for compile admission or the package cache. It is inert unless
/// the variable is set, and it only records: the counts and the messages come
/// from the build that just ran. Timestamps let a reader derive each phase's
/// elapsed time by subtracting consecutive entries.
fn record_phase(emit: bool, counts: BuildCounts, stderr: &[u8]) -> Result<()> {
    let Some(path) = std::env::var_os(TELEMETRY_VARIABLE) else {
        return Ok(());
    };
    let telemetry = PathBuf::from(path);
    let name = telemetry.file_name().with_context(|| {
        format!(
            "fixture telemetry path {} should name a file",
            telemetry.display()
        )
    })?;
    let parent = telemetry
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let directory = Dir::open_ambient_dir(parent, ambient_authority())
        .context("open the fixture telemetry directory")?;
    let mut file = directory
        .open_with(name, OpenOptions::new().create(true).append(true))
        .context("open the fixture telemetry file")?;
    let stderr_text = String::from_utf8_lossy(stderr);
    writeln!(
        file,
        "at_ms={} emit={emit} {} admission_lines={} package_cache_lines={}",
        unix_millis(),
        counts.summary(),
        count_matching_lines(&stderr_text, "[build-limits]"),
        count_matching_lines(&stderr_text, "waiting for file lock"),
    )
    .context("append the fixture build telemetry")
}

/// Wall-clock milliseconds, so phases can be ordered and timed.
fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis())
}

/// Count the lines of `text` that contain `needle`.
fn count_matching_lines(text: &str, needle: &str) -> usize {
    text.lines().filter(|line| line.contains(needle)).count()
}

/// Remove only the fixture's build output in its private target directory.
pub(super) fn clean_fixture(cargo: &FixtureCargo) -> Result<()> {
    let mut command = Command::new("cargo");
    cargo_environment::CargoEnvironment::build().apply(&mut command);
    let output = process_runner::run_with_policy(
        command
            .args(["clean", "-p", "orthohelp_fixture", "--target-dir"])
            .arg(&cargo.target_directory)
            .current_dir(&cargo.repository),
        process_runner::Operation("clean identifier artefact fixture outputs"),
        cargo.bounded_policy(
            process_runner::Policy::build(),
            "clean identifier artefact fixture outputs",
        )?,
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
pub(super) fn artefact_path(out_dir: &Path) -> Result<Option<PathBuf>> {
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
pub(super) fn artefact(out_dir: &Path) -> Result<String> {
    let directory = Dir::open_ambient_dir(out_dir, ambient_authority())
        .context("open fixture build-script output directory")?;
    directory
        .read_to_string("ortho-config/cli-identifiers.json")
        .context("read identifier artefact")
}

fn retain_runner_error(error: anyhow::Error, operation: &str) -> anyhow::Error {
    if process_runner::deadline_exceeded(&error) {
        error.context(format!("{operation} exceeded its overall deadline"))
    } else if process_runner::cleanup_incomplete(&error) {
        error.context(format!("{operation} cleanup was not confirmed"))
    } else {
        error.context(format!("{operation} subprocess failed"))
    }
}
