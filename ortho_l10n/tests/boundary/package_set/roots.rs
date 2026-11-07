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

use super::super::fixture_cargo::{BOUNDARY_ROOT, repository_root};
use super::{PACKAGING_INVOCATIONS, package_sources};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use std::sync::{Mutex, MutexGuard, OnceLock, TryLockError};
use std::thread;
use std::time::{Duration, Instant};

/// Directory `cargo package` writes into, relative to the shared target root.
const PACKAGE_DIRECTORY: &str = "package";

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

/// Process-local state for the packaged roots used by boundary contracts.
///
/// Successful packaging is shared by every caller in the test process. An
/// unbounded packaging failure is retained so callers do not repeat failed
/// Cargo work; a failure from a sequence-bounded attempt is caller-specific and
/// must not prevent a later caller with remaining budget from retrying. This
/// cache belongs only to this package-set module and is not a general-purpose
/// cache for other test fixtures.
struct PackagedSetCache {
    roots: OnceLock<Vec<PackagedCrate>>,
    failure: OnceLock<String>,
    preparation: Mutex<()>,
}

impl PackagedSetCache {
    /// Create an empty process-local cache.
    const fn new() -> Self {
        Self {
            roots: OnceLock::new(),
            failure: OnceLock::new(),
            preparation: Mutex::new(()),
        }
    }

    /// Package once, caching successful results and only unbounded failures.
    fn prepare_with(
        &self,
        sequence_deadline: Option<Instant>,
        build: impl FnOnce(Option<Instant>) -> Result<Vec<PackagedCrate>>,
    ) -> Result<()> {
        if self.is_ready()? {
            return Ok(());
        }
        let _preparation = lock_preparation(&self.preparation, sequence_deadline)?;
        if self.is_ready()? {
            return Ok(());
        }
        ensure_deadline_remaining(sequence_deadline)?;
        self.build_and_cache(sequence_deadline, build)
    }

    /// Whether roots are ready, or the retained unbounded failure if packaging failed.
    fn is_ready(&self) -> Result<bool> {
        match (self.roots.get(), self.failure.get()) {
            (Some(_), _) => Ok(true),
            (_, Some(error)) => anyhow::bail!("{error}"),
            (None, None) => Ok(false),
        }
    }

    /// Run the package recipe and store only caller-independent outcomes.
    fn build_and_cache(
        &self,
        sequence_deadline: Option<Instant>,
        build: impl FnOnce(Option<Instant>) -> Result<Vec<PackagedCrate>>,
    ) -> Result<()> {
        match build(sequence_deadline) {
            Ok(roots) => self.cache_roots(roots),
            Err(error) => self.report_build_failure(sequence_deadline, &error),
        }
    }

    /// Retain one successful package result for later callers.
    fn cache_roots(&self, roots: Vec<PackagedCrate>) -> Result<()> {
        self.roots
            .set(roots)
            .map_err(|_| anyhow::anyhow!("packaged roots were initialized concurrently"))
    }

    /// Report a build failure, retaining it only when it has no caller deadline.
    fn report_build_failure(
        &self,
        sequence_deadline: Option<Instant>,
        error: &anyhow::Error,
    ) -> Result<()> {
        let message = format!("{error:#}");
        if sequence_deadline.is_none() {
            self.failure.get_or_init(|| message.clone());
        }
        anyhow::bail!("{message}")
    }
}

/// Acquire the preparation lock within an optional caller deadline.
fn lock_preparation(
    preparation: &Mutex<()>,
    sequence_deadline: Option<Instant>,
) -> Result<MutexGuard<'_, ()>> {
    sequence_deadline.map_or_else(
        || {
            preparation
                .lock()
                .map_err(|error| anyhow::anyhow!("package preparation lock is poisoned: {error}"))
        },
        |deadline| lock_before_deadline(preparation, deadline),
    )
}

/// Refuse to start packaging after a caller's enclosing sequence deadline.
fn ensure_deadline_remaining(sequence_deadline: Option<Instant>) -> Result<()> {
    if let Some(deadline) = sequence_deadline {
        anyhow::ensure!(
            deadline > Instant::now(),
            "packaged set could not start before its enclosing sequence deadline"
        );
    }
    Ok(())
}

/// Wait for the package operation without exceeding a caller's deadline.
fn lock_before_deadline(
    preparation: &Mutex<()>,
    sequence_deadline: Instant,
) -> Result<MutexGuard<'_, ()>> {
    const POLL_INTERVAL: Duration = Duration::from_millis(10);

    loop {
        match preparation.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(error)) => {
                anyhow::bail!("package preparation lock is poisoned: {error}");
            }
            Err(TryLockError::WouldBlock) => {
                let remaining = sequence_deadline.saturating_duration_since(Instant::now());
                anyhow::ensure!(
                    !remaining.is_zero(),
                    "packaged set could not start before its enclosing sequence deadline"
                );
                thread::sleep(remaining.min(POLL_INTERVAL));
            }
        }
    }
}

/// The packaged roots, materialized once per test process.
static PACKAGED: PackagedSetCache = PackagedSetCache::new();

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
/// reads the same directory. An unbounded failure is cached too, so a broken
/// packaging step is reported once rather than retried by every contract. A
/// sequence-bounded failure is not cached because that deadline belongs only
/// to its caller.
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
/// # Errors
///
/// Returns an error when either packaging invocation fails or when one of the
/// expected unpacked roots is missing afterwards.
pub(crate) fn prepare_packaged_set() -> Result<()> {
    prepare_packaged_set_with_deadline(None)
}

/// Package and unpack this release before an enclosing test sequence deadline.
///
/// # Errors
///
/// Returns an error when packaging fails or the sequence deadline expires.
pub(crate) fn prepare_packaged_set_until(sequence_deadline: Instant) -> Result<()> {
    prepare_packaged_set_with_deadline(Some(sequence_deadline))
}

fn prepare_packaged_set_with_deadline(sequence_deadline: Option<Instant>) -> Result<()> {
    PACKAGED.prepare_with(sequence_deadline, build_packaged_set)
}

/// The materialized packaged set.
///
/// # Errors
///
/// Returns an error when [`prepare_packaged_set`] has not run, or when it
/// failed. Both are reported rather than retried: this function cannot package
/// anything, by design.
fn packaged_set() -> Result<&'static [PackagedCrate]> {
    if let Some(roots) = PACKAGED.roots.get() {
        return Ok(roots);
    }
    if let Some(error) = PACKAGED.failure.get() {
        anyhow::bail!("{error}");
    }
    anyhow::bail!("the packaged set was never prepared; call prepare_packaged_set first")
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
fn build_packaged_set(sequence_deadline: Option<Instant>) -> Result<Vec<PackagedCrate>> {
    let repository = repository_root()?;
    let target = repository.join(BOUNDARY_ROOT).join("packaging-target");
    Dir::create_ambient_dir_all(&target, cap_std::ambient_authority())
        .with_context(|| format!("create packaging target {target}"))?;

    for invocation in &PACKAGING_INVOCATIONS {
        package_sources(&repository, &target, invocation, sequence_deadline)?;
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

/// Assert every packaged root and archive exists.
///
/// A pure query over the prepared set: it inspects the filesystem, changes
/// nothing, and does not trigger packaging. A caller that expected packaging
/// to have happened must call [`prepare_packaged_set`] first, and gets a
/// "never prepared" error rather than a silent re-run if it did not.
///
/// # Errors
///
/// Returns an error naming the first missing artefact, or when the set was
/// never prepared.
pub(crate) fn assert_packaged_set_complete() -> Result<()> {
    let unpacked = unpacked_directory()?;
    for packaged in packaged_set()? {
        ensure!(
            packaged.manifest().is_file(),
            "{} has no unpacked manifest; the packaging step did not produce it",
            packaged.manifest(),
        );
        let archive = unpacked.join(format!("{}-{}.crate", packaged.name, packaged.version));
        ensure!(archive.is_file(), "{archive} was not written");
    }
    Ok(())
}

#[cfg(test)]
#[path = "roots_tests.rs"]
mod tests;
