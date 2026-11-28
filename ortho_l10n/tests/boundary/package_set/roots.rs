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
use super::{PACKAGING_INVOCATIONS, package_sources, prepared};
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::fs_utf8::Dir;
use std::sync::OnceLock;

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

/// The packaged set, admitted once per test process by [`prepare_packaged_set`].
///
/// This is a *store*, not a lookup, and it is module-private for that reason:
/// a caller reading it is a caller that cannot spawn packaging or Cargo, which
/// is what keeps the contracts' own queries free of side effects.
static PACKAGED: OnceLock<Result<Vec<PackagedCrate>, String>> = OnceLock::new();

/// Admit the packaged set that `make boundary-prepare` produced.
///
/// This is the command half of the module, and it no longer packages. The two
/// verified `cargo package` invocations run in the preparation step
/// ([`begin_preparation`]), outside every timed test, because verification
/// compiles the packaged closure on every run and a warm target does not
/// shorten it. Here the contract checks the preparation record against the
/// sources, the lockfile, the toolchain and the archives on disk, and refuses
/// when it does not match. Nothing is spawned except the two toolchain
/// version probes, and nothing touches the network.
///
/// Because no contract replaces a packaged root, the cross-process hazard that
/// used to need `binary(boundary)` serialized for packaging (one process
/// unpacking over a root another was reading) no longer exists. The group
/// stays, since the consumer builds share a target directory.
///
/// The first caller checks; every later caller in the process reads the cached
/// verdict, failures included, so a missing record is reported once.
///
/// # Errors
///
/// Returns an error naming `make boundary-prepare` when the suite has not been
/// prepared or the preparation is stale.
pub(crate) fn prepare_packaged_set() -> Result<()> {
    let packaged =
        PACKAGED.get_or_init(|| admit_prepared_set().map_err(|error| format!("{error:#}")));
    packaged
        .as_deref()
        .map(|_roots| ())
        .map_err(|error| anyhow::anyhow!("{error}"))
}

/// A preparation that has packaged and verified the set but not yet published
/// its record.
///
/// The preparation step packages first, builds every consumer shape against the
/// unpacked roots, and only then publishes, so a step that stops anywhere
/// before the end leaves no record. [`begin_preparation`] makes one.
#[derive(Debug)]
pub(crate) struct Preparation {
    /// The sources and toolchain as they were before packaging began.
    snapshot: prepared::Snapshot,
    /// The packaging target the roots were unpacked into.
    target: Utf8PathBuf,
}

impl Preparation {
    /// The directory holding every unpacked root.
    pub(crate) fn unpacked(&self) -> Utf8PathBuf {
        self.target.join(PACKAGE_DIRECTORY)
    }

    /// Publish the record, then admit the set through the check a timed
    /// contract uses.
    ///
    /// Admission here means a record that does not round-trip fails in the
    /// preparation step, where the cause is visible, and not in a later
    /// contract.
    ///
    /// # Errors
    ///
    /// Returns an error when the archives or roots cannot be digested, the
    /// record cannot be written, or the written record is refused.
    pub(crate) fn publish(self) -> Result<()> {
        prepared::write(self.snapshot, &self.target)?;
        prepare_packaged_set()
    }
}

/// Run both verified packaging invocations, outside any timed test.
///
/// Runs under [`Budget::preparation`]. It discards the old record first, and
/// takes the digest of every source before packaging starts, so a source edited
/// while `cargo package` runs is recorded as it was before and the first check
/// refuses it as stale. The record itself is published by
/// [`Preparation::publish`], after the consumer builds.
///
/// # Errors
///
/// Returns an error when the old record cannot be discarded, a source cannot be
/// digested, or either packaging invocation fails.
pub(crate) fn begin_preparation(budget: Budget) -> Result<Preparation> {
    let repository = repository_root()?;
    let target = packaging_target(&repository);
    prepared::invalidate(&target)?;
    Dir::create_ambient_dir_all(&target, cap_std::ambient_authority())
        .with_context(|| format!("create packaging target {target}"))?;
    let toolchain = prepared::toolchain_identity(&repository)?;
    let snapshot = prepared::Snapshot::capture(&repository, &toolchain)?;
    run_packaging_invocations(&repository, &target, budget)?;
    Ok(Preparation { snapshot, target })
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

/// The directory the packaging invocations build and unpack into.
fn packaging_target(repository: &camino::Utf8Path) -> Utf8PathBuf {
    repository.join(BOUNDARY_ROOT).join("packaging-target")
}

/// Check the preparation record and collect the unpacked roots it describes.
fn admit_prepared_set() -> Result<Vec<PackagedCrate>> {
    let repository = repository_root()?;
    let target = packaging_target(&repository);
    let toolchain = prepared::toolchain_identity(&repository)?;
    prepared::check(&repository, &target, &toolchain)?;
    Ok(packaged_roots(&target.join(PACKAGE_DIRECTORY)))
}

/// Run each packaging invocation, in order, into `target`.
fn run_packaging_invocations(
    repository: &camino::Utf8Path,
    target: &camino::Utf8Path,
    budget: Budget,
) -> Result<()> {
    for invocation in &PACKAGING_INVOCATIONS {
        package_sources(repository, target, invocation, budget)?;
    }
    Ok(())
}

/// The unpacked root of every packaged crate beneath `package_directory`.
fn packaged_roots(package_directory: &camino::Utf8Path) -> Vec<PackagedCrate> {
    PACKAGED_CRATES
        .into_iter()
        .map(|(name, version)| PackagedCrate {
            name,
            version,
            root: package_directory.join(format!("{name}-{version}")),
        })
        .collect()
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
