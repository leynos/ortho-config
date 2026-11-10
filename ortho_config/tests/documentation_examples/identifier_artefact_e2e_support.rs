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
//!
//! The lease has to outlive an interrupted build as well as its holder. Each
//! nested Cargo invocation inherits a duplicate of the lease handle as its
//! standard input, so the lock stays held while any compiler beneath that
//! invocation can still write to the target. A helper that dies abruptly
//! therefore cannot hand the target to a replacement process while its own
//! Cargo tree is still running; see [`fixture_cargo`] and the lease module.

#[path = "identifier_artefact_cargo_messages.rs"]
mod cargo_messages;
#[path = "identifier_artefact_cargo_messages_property_tests.rs"]
#[cfg(test)]
mod cargo_messages_property_tests;
#[path = "identifier_artefact_descendants.rs"]
mod descendants;
#[path = "identifier_artefact_fixture_build.rs"]
mod fixture_build;
#[path = "identifier_artefact_fixture_tests.rs"]
#[cfg(test)]
mod fixture_tests;
#[path = "identifier_artefact_lease.rs"]
mod lease;
#[path = "identifier_artefact_telemetry.rs"]
mod telemetry;

pub(super) use self::descendants::{Descendants, snapshot_descendants};
pub(super) use self::fixture_build::{artefact, artefact_path, build_fixture, clean_fixture};
pub(super) use self::lease::{TargetLease, acquire};

use self::telemetry::TelemetrySink;
use super::{cargo_environment, process_runner, scenario};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

const CACHEDIR_TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55\n# This file is a cache directory tag created by cargo.\n# For information about cache directory tags see https://bford.info/cachedir/\n";

/// Names the fixture's private Cargo target.
///
/// Nested under the workspace target so `cargo clean` of the workspace removes
/// it, and named so consecutive runs reuse the dependency build.
const TARGET_DIRECTORY_NAME: &str = "identifier-artefact-target";

/// The fixture package and its private target, before the cross-process lease.
///
/// Discovery runs `cargo metadata`, which only reads the workspace and creates
/// nothing, so it is safe to perform while another test process holds the
/// lease. Splitting discovery from [`FixtureLocation::lease`] lets a
/// regression observe that a second process has reached the target and is
/// blocked on the lease, rather than merely slow to start, and keeps every
/// write to the target behind the lease.
pub(super) struct FixtureLocation {
    repository: PathBuf,
    target_directory: PathBuf,
    package_id: String,
}

impl FixtureLocation {
    /// The private target this location will lease.
    ///
    /// Reading the path creates nothing; a regression uses it to observe a
    /// helper's target without being the process that leases it.
    pub(super) fn target_directory(&self) -> &Path {
        &self.target_directory
    }

    /// Take the whole-test lease on the private target and build the fixture.
    ///
    /// The lease waits until every other leaseholder has released the target,
    /// so a concurrent test process cannot interleave its
    /// `cargo clean -p orthohelp_fixture` with this test's build or artefact
    /// read. [`serial_test`] and nextest's process group cannot provide this:
    /// they coordinate one runner's tests, and `#[serial]` is a thread lock
    /// inside one process.
    ///
    /// `deadline` is the envelope captured before discovery, so metadata, this
    /// wait and every later build share the one allowance nextest grants the
    /// test. Starting a second envelope here would leave the total unbounded:
    /// discovery's 30 second cap and the wait's full remainder could add up
    /// past nextest's per-test ceiling, and the test would be killed before
    /// the wait could name itself.
    ///
    /// `telemetry` is supplied by the caller rather than read from the
    /// environment here, so the fixture build has one explicit sink and a
    /// regression can drive the record format without mutating the process
    /// environment.
    pub(super) fn lease(
        self,
        deadline: Instant,
        telemetry: Option<TelemetrySink>,
    ) -> Result<FixtureCargo> {
        let lease = acquire(&self.target_directory, deadline)?;
        prepare_target(&self.target_directory)?;
        Ok(FixtureCargo {
            repository: self.repository,
            target_directory: self.target_directory,
            package_id: self.package_id,
            deadline,
            telemetry,
            lease,
        })
    }
}

/// Mark the private target as Cargo cache output.
///
/// This is the one write discovery must not perform. The lease creates the
/// target and this call tags it, both after the lease is held, so a second
/// process cannot prepare the same directory beside this one.
fn prepare_target(target: &Path) -> Result<()> {
    Dir::open_ambient_dir(target, ambient_authority())
        .context("open the private Cargo target")?
        .write("CACHEDIR.TAG", CACHEDIR_TAG.as_bytes())
        .context("write Cargo cache-directory tag")
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
    telemetry: Option<TelemetrySink>,
    lease: TargetLease,
}

impl FixtureCargo {
    /// The phase telemetry sink, when the evidence run asked for one.
    const fn telemetry(&self) -> Option<&TelemetrySink> {
        self.telemetry.as_ref()
    }

    /// A duplicate of the lease handle for a nested Cargo child to inherit.
    ///
    /// The child's descendants keep the target leased while they live, so an
    /// interrupted helper cannot release the target to a replacement process
    /// before its compilers have stopped writing.
    pub(super) fn inherited_lease(&self) -> Result<Stdio> {
        self.lease.inherited_handle()
    }

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
    let deadline = scenario::deadline();
    let telemetry = TelemetrySink::from_environment()?;
    locate_fixture(deadline)?.lease(deadline, telemetry)
}

/// Discover the fixture package and locate its private Cargo target.
///
/// This is the half of [`fixture_cargo`] that is safe to run concurrently with
/// another process's lease: it only reads workspace metadata and returns the
/// path the lease will create. Nothing is created or written here, so a second
/// process can run discovery while the first holds the lease without either
/// touching the target.
///
/// `deadline` is the test's whole envelope, and this call is already charged
/// to it. Discovery is capped at 30 seconds; the lease wait that follows takes
/// what is left, so the two cannot together outlast nextest's per-test
/// allowance.
pub(super) fn locate_fixture(deadline: Instant) -> Result<FixtureLocation> {
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
    location_from_metadata(repository, &metadata)
}

/// Build the fixture location from parsed metadata, reading only.
///
/// Kept separate from the command so a regression can hand it a fresh target
/// path and observe that discovery creates neither the directory nor its tag.
fn location_from_metadata(repository: PathBuf, metadata: &Value) -> Result<FixtureLocation> {
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
    Ok(FixtureLocation {
        repository,
        target_directory: private_target(Path::new(workspace_target)),
        package_id: package_id.to_owned(),
    })
}

/// The fixture's private target, nested under the workspace target.
///
/// A pure path computation: the directory is created by the lease, not here,
/// so discovery can run beside another leaseholder without writing.
pub(super) fn private_target(workspace_target: &Path) -> PathBuf {
    workspace_target.join(TARGET_DIRECTORY_NAME)
}
