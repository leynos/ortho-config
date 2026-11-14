//! The unpacked package roots the boundary contracts read.
//!
//! `package_set` owns the recipe: which crates to package, which sibling
//! sources to patch, and how long each invocation may run. This module owns the
//! result of that recipe - the roots `cargo package --verify` unpacks, the
//! version they share, and the cache that materializes them once per process.
//!
//! The split is by responsibility rather than by size. The recipe is read by
//! `boundary_packaging_test.py`, which checks its invocation count and deadline
//! against nextest's per-test allowance, so it belongs in one clearly readable
//! place. Everything here is instead read by the consumer and manifest
//! contracts, which care about paths and normalized manifests rather than about
//! how the archives were produced.

use super::super::fixture_cargo::{BOUNDARY_ROOT, Budget, repository_root};
use super::lease::{PackagingLease, Prepared, prepare};
use super::record::{clear_record, record_complete};
use super::run_guard::{RunGuard, hold_run};
use super::{PACKAGING_INVOCATIONS, package_sources};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use std::sync::OnceLock;
use std::time::Instant;

/// Directory `cargo package` writes into, relative to the shared target root.
const PACKAGE_DIRECTORY: &str = "package";

/// Environment variable nextest exports to every test it runs.
const RUN_ID_VARIABLE: &str = "NEXTEST_RUN_ID";

/// The value the completion record must name for this process to reuse it.
///
/// `None` when no run identifier is exported, which is the single-process
/// runners: there the process-local cache is what deduplicates, and a record
/// made by an earlier run must not be trusted.
///
/// This is the composition edge, so the environment is read here and the
/// result is passed inward. [`prepare`] and the record helpers take the scope
/// as an argument and consult no process-global state, which is what lets their
/// regressions drive them without mutating the environment.
fn run_scope() -> Option<String> {
    std::env::var(RUN_ID_VARIABLE)
        .ok()
        .filter(|id| !id.is_empty())
}

/// One unpacked package root, with the paths a consumer patch needs.
#[derive(Debug, Clone)]
pub(crate) struct PackagedCrate {
    /// Crate name as spelled in a manifest.
    pub(crate) name: &'static str,
    /// Version every packaged crate in this release shares.
    pub(crate) version: &'static str,
    /// Unpacked root, holding `Cargo.toml` and `src/`.
    pub(crate) root: Utf8PathBuf,
}

impl PackagedCrate {
    /// Path of the unpacked root's normalized manifest.
    pub(crate) fn manifest(&self) -> Utf8PathBuf {
        self.root.join("Cargo.toml")
    }

    /// Read the unpacked root's normalized manifest.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest cannot be read.
    pub(crate) fn read_manifest(&self) -> Result<String> {
        let path = self.manifest();
        let directory = Dir::open_ambient_dir(&self.root, cap_std::ambient_authority())
            .with_context(|| format!("open packaged root {}", self.root))?;
        directory
            .read_to_string("Cargo.toml")
            .with_context(|| format!("read packaged manifest {path}"))
    }
}

/// The version every crate in this release shares.
///
/// Taken from the crate under test rather than written out, because
/// `cargo package` names the unpacked root and the archive after the version in
/// the manifest. A release bump would otherwise leave these contracts looking
/// for `0.9.0` directories that no longer exist, and every packaged contract
/// would fail for a reason unrelated to the boundary claim.
pub(crate) const PACKAGED_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The three crates this release publishes, in dependency order.
pub(crate) const PACKAGED_CRATES: [(&str, &str); 3] = [
    ("ortho_l10n", PACKAGED_VERSION),
    ("ortho_config_macros", PACKAGED_VERSION),
    ("ortho_config", PACKAGED_VERSION),
];

/// The packaged set, materialized once per test process by
/// [`prepare_packaged_set`].
///
/// This is a *store*, not a lookup, and it is module-private for that reason:
/// a caller reading it is a caller that cannot spawn packaging or Cargo, which
/// is what keeps the contracts' own queries free of side effects.
static PACKAGED: OnceLock<Result<Vec<PackagedCrate>, String>> = OnceLock::new();

/// The boundary's run guard, held until this process exits.
///
/// A `static` rather than a local because the lock's promise is "no other run
/// will touch these roots while I am alive", and the roots outlive
/// [`build_packaged_set`] by the whole life of the contracts reading them.
/// Dropping it at the end of the build would let a second run start replacing
/// the set while the first is still reading it, which is the defect this
/// exists to prevent.
static RUN_GUARD: OnceLock<RunGuard> = OnceLock::new();

/// Package and unpack this release's crates, once per test process.
///
/// This is the command half of the module. It runs the packaging invocations
/// and populates the store; every other function here and in the contract
/// modules only reads the result. Keeping the two apart is what lets a
/// contract say "resolve this consumer" without also meaning "package the
/// workspace", and it means the packaging boundary appears exactly once in the
/// suite.
///
/// The first caller packages and unpacks; every later caller is a no-op that
/// reads the same directory. A failure is cached too, so a broken packaging
/// step is reported once rather than retried by every contract.
///
/// That cache is process-local, and nextest runs each test in its own process,
/// so in a full run several of these packaged contracts package the same target
/// directory side by side. Cargo's own lock does not cover that: it does not
/// hold readers out while a package root is removed and unpacked, so one
/// process can replace a root another is reading. `binary(boundary)` is
/// therefore serialized as its own nextest test group; see
/// `.config/nextest.toml` and "Nextest test-group serialization" in
/// `docs/developers-guide.md`. Without that entry this function is safe only
/// under `make boundary`, which runs one process.
///
/// The first caller spends `budget` on the two packaging invocations; later
/// callers in the same process are no-ops that do not consume it. Across
/// processes, [`build_packaged_set`] takes a lease on the packaging target and
/// reuses the roots once per nextest run, so the twelve invocations a full
/// `boundary` run would otherwise make become two. See [`lease`] for why the
/// reuse is scoped to one run rather than to the target directory.
///
/// # Errors
///
/// Returns an error when either packaging invocation fails, when the
/// contract's shared allowance is spent before one can start, when another
/// process holds the packaging lease past the allowance, or when one of the
/// expected unpacked roots is missing afterwards.
pub(crate) fn prepare_packaged_set(budget: Budget) -> Result<()> {
    let packaged =
        PACKAGED.get_or_init(|| build_packaged_set(budget).map_err(|error| format!("{error:#}")));
    packaged
        .as_deref()
        .map(|_roots| ())
        .map_err(|error| anyhow::anyhow!("{error}"))
}

/// The materialized packaged set.
///
/// # Errors
///
/// Returns an error when [`prepare_packaged_set`] has not run, or when it
/// failed. Both are reported rather than retried: this function cannot package
/// anything, by design.
fn packaged_set() -> Result<&'static [PackagedCrate]> {
    let cached = PACKAGED
        .get()
        .context("the packaged set was never prepared; call prepare_packaged_set first")?;
    cached
        .as_deref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

/// Look up one packaged crate by name.
///
/// A pure lookup over a set [`prepare_packaged_set`] already materialized.
///
/// # Errors
///
/// Returns an error when the crate is not in the packaged set.
pub(crate) fn packaged_crate(name: &str) -> Result<&'static PackagedCrate> {
    packaged_set()?
        .iter()
        .find(|packaged| packaged.name == name)
        .with_context(|| format!("the packaged set does not contain {name}"))
}

/// The directory holding all unpacked roots.
///
/// A pure derivation from the prepared set, like [`packaged_crate`].
///
/// # Errors
///
/// Returns an error when the set is empty or was never prepared.
pub(crate) fn unpacked_directory() -> Result<Utf8PathBuf> {
    let first = packaged_set()?
        .first()
        .context("the packaged set is empty")?;
    let parent = first
        .root
        .parent()
        .context("a packaged root has no parent directory")?;
    Ok(parent.to_path_buf())
}

/// Run both packaging invocations and collect the unpacked roots.
///
/// The lease decides whether they run at all: a contract that finds this
/// run's roots already complete returns the paths without packaging, so the
/// work is done once per run rather than once per test process.
///
/// # Why the fill is clamped to the contract's *original* allowance
///
/// The after-run's six "22 second deadline" failures came from a contract that
/// won the lease with 22 s of its 900 s allowance left, so its fill was clamped
/// to that remainder. Restarting the clock after the lease wait would give the
/// fill a fresh 900 s, but it would also let a contract that waited 878 s then
/// pack for 440 s - 1,318 s inside a 960 s nextest allowance. nextest would
/// stop the test first, and the error naming the invocation would never be
/// printed, which is exactly what [`Budget`]'s module comment exists to
/// prevent. The clamp is therefore correct. What it exposes is that the
/// measured pair - 802 s plus 342 s, about 1,144 s - cannot fit a 900 s
/// allowance under either bound, and that is recorded in
/// `docs/execplans/harden-boundary-deadline-evidence-20261113.md` rather than
/// repaired here by raising a figure.
fn build_packaged_set(budget: Budget) -> Result<Vec<PackagedCrate>> {
    let repository = repository_root()?;
    let boundary = repository.join(BOUNDARY_ROOT);
    let target = boundary.join("packaging-target");

    let deadline = Instant::now() + budget.remaining();
    // Taken before anything in the target is read or written, and kept for the
    // life of the process by `RUN_GUARD`. Without it a second run could begin
    // replacing the roots this one is about to read, which is exactly what
    // `Prepared::Reused` promises cannot happen.
    let guard = hold_run(&boundary, deadline)?;
    let _held = RUN_GUARD.get_or_init(|| guard);

    let scope = run_scope();
    match prepare(&target, scope.as_deref(), deadline)? {
        Prepared::Reused => {}
        Prepared::Owned { lease } => {
            package_under_lease(&lease, scope.as_deref(), || {
                package_all(&repository, &target, budget)
            })?;
        }
    }

    let package_directory = target.join(PACKAGE_DIRECTORY);
    let mut roots = Vec::new();
    for (name, version) in PACKAGED_CRATES {
        roots.push(PackagedCrate {
            name,
            version,
            root: package_directory.join(format!("{name}-{version}")),
        });
    }
    Ok(roots)
}

/// Replace the roots under a held lease, recording the result.
///
/// The order is the whole point, and it is why this is a function rather than
/// three statements inline: the record is cleared *before* packaging starts and
/// written *after* it succeeds, both while the lease is held. A reader can
/// therefore never see a record describing roots that are half-replaced, and a
/// failed step leaves no record at all.
///
/// The packaging work is a closure so a test can drive the sequence without
/// running `cargo package`, and can make it fail.
///
/// # Errors
///
/// Returns the packaging error unchanged, or an error from clearing or writing
/// the record.
fn package_under_lease(
    lease: &PackagingLease,
    scope: Option<&str>,
    package: impl FnOnce() -> Result<()>,
) -> Result<()> {
    clear_record(lease)?;
    package()?;
    // Recorded only for a named run: without one, the process-local cache is
    // what deduplicates, and a record this process cannot reuse is a file the
    // next run has to distrust rather than a saving.
    scope.map_or_else(|| Ok(()), |this_run| record_complete(lease, this_run))
}

/// Run every packaging invocation, leaving the roots materialized.
fn package_all(repository: &Utf8Path, target: &Utf8Path, budget: Budget) -> Result<()> {
    for invocation in &PACKAGING_INVOCATIONS {
        package_sources(repository, target, invocation, budget)?;
    }
    Ok(())
}

/// Assert every packaged root and archive exists.
///
/// A pure query over the prepared set: it inspects the filesystem, changes
/// nothing, and does not trigger packaging. A caller that expected packaging
/// to have happened must call [`prepare_packaged_set`] first, and gets a
/// "never prepared" error rather than a silent re-run if it did not.
///
/// # Errors
///
/// Returns an error naming the first missing artefact, when an artefact's
/// existence cannot be established, or when the set was never prepared.
pub(crate) fn assert_packaged_set_complete() -> Result<()> {
    let unpacked = unpacked_directory()?;
    for packaged in packaged_set()? {
        require_file(&packaged.manifest(), "unpacked manifest")?;
        let archive = unpacked.join(format!("{}-{}.crate", packaged.name, packaged.version));
        require_file(&archive, "packaged archive")?;
    }
    Ok(())
}

/// Require `path` to be a file, reporting why when that cannot be established.
///
/// `Utf8Path::is_file` answers `false` for every failure, so a permission or
/// handle fault would be reported as "the packaging step did not produce it" -
/// sending a reader to look for a missing artefact that may well be there.
/// `try_exists` keeps the two apart, and the directory case is named separately
/// because it is the one a reader would otherwise misread as absence.
fn require_file(path: &Utf8Path, role: &str) -> Result<()> {
    ensure!(
        path.try_exists()
            .with_context(|| format!("establish whether {path} exists"))?,
        "the {role} {path} is absent; the packaging step did not produce it"
    );
    ensure!(
        path.is_file(),
        "the {role} {path} exists but is not a regular file"
    );
    Ok(())
}

#[cfg(test)]
#[path = "roots_tests.rs"]
mod roots_tests;
