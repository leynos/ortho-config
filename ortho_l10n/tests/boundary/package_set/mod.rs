//! Materializes the three crates as a stranger would receive them.
//!
//! A path dependency in this workspace proves nothing about the published
//! graph. Cargo rewrites a member's `path` dependency into a registry version
//! requirement when it packages, and a stranger then resolves that version
//! requirement from crates.io with no workspace in sight. The contracts here
//! exercise the second half of that: the packaged manifests, the packaged
//! sources, and the exact edges between them.
//!
//! There is no network step. `cargo package` is run offline against the shared
//! package cache, and the resulting `.crate` files are unpacked into the same
//! target directory. That produces the *stranger's* manifests - the normalized
//! `Cargo.toml` with path dependencies rewritten - while leaving the sources
//! identical to the workspace's.
//!
//! # The two-invocation recipe
//!
//! `ortho_config` and `ortho_config_macros` each depend on the others by
//! version, and no published version exists yet. Packaging them therefore needs
//! the same-version edges patched to the sibling sources, and packaging all
//! three in one invocation fails: Cargo verifies a crate against the lockfile
//! it just generated, and the sibling that the first invocation patched is not
//! the one the second invocation's lockfile records. Two invocations, each
//! patching the crate it does not package, produce all three roots.
//!
//! # Cost
//!
//! The two invocations verify what they package, so they compile the closure:
//! about 35 seconds from warm caches on this host, and a few hundred megabytes
//! of target directory. The result is shared across the contract tests and
//! reused between runs, because the validation is a property of the sources,
//! not of the test that reads it.

use super::fixture_cargo::{
    FixtureCargo, cargo_binary, inherited_toolchain, repository_root, run_with_deadline,
    toml_string,
};
use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use std::process::Command;
use std::sync::OnceLock;
use std::time::Duration;

/// Longest either packaging invocation may run.
///
/// Each invocation is bounded on its own, and every entry in
/// [`PACKAGING_INVOCATIONS`] shares one test, so all of them have to fit
/// inside nextest's per-test allowance with room for the root checks around
/// them: 2 x 240 s is 480 s of the 600 s base allowance, leaving 120 s. The
/// figure is derived from the allowance rather than chosen for comfort,
/// because a bound that does not fit never fires: nextest stops the test
/// first, and the error naming the invocation that hung is never printed.
///
/// The second invocation reuses the first's target directory and is much
/// shorter. For scale, the pair took 244 s on this host with the packaging
/// target removed, which is an upper bound rather than a build time: it
/// includes waiting for compile-admission slots, which the child inherits. So
/// 240 s per invocation is generous against the whole pair rather than tight
/// against either half.
const PACKAGE_DEADLINE: Duration = Duration::from_secs(240);

/// The `cargo package` invocations that together produce all three roots.
///
/// Each entry names the crates it packages and the sibling it patches to the
/// workspace sources. They are data rather than two call statements because
/// the per-test allowance is shared between them: `boundary_packaging_test.py`
/// reads this array's length and [`PACKAGE_DEADLINE`] to check that the whole
/// set can reach its own deadlines before nextest stops the test. The array
/// length is part of the type, so rustc checks it against the initializers.
const PACKAGING_INVOCATIONS: [(&[&str], &str); 2] = [
    (&["ortho_l10n", "ortho_config"], "ortho_config_macros"),
    (&["ortho_config_macros"], "ortho_l10n"),
];

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

/// Materialize (once per test process) and return the packaged set.
///
/// The first caller packages and unpacks; every later caller reads the same
/// directory. A failure is cached too, so a broken packaging step is reported
/// once rather than retried by every contract.
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
pub(crate) fn packaged_set() -> Result<&'static [PackagedCrate]> {
    static PACKAGED: OnceLock<Result<Vec<PackagedCrate>, String>> = OnceLock::new();
    PACKAGED
        .get_or_init(|| build_packaged_set().map_err(|error| format!("{error:#}")))
        .as_deref()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

/// Look up one packaged crate by name.
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
/// # Errors
///
/// Returns an error when the packaged set cannot be materialized.
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
fn build_packaged_set() -> Result<Vec<PackagedCrate>> {
    let repository = repository_root()?;
    let target = repository
        .join(super::fixture_cargo::BOUNDARY_ROOT)
        .join("packaging-target");
    Dir::create_ambient_dir_all(&target, cap_std::ambient_authority())
        .with_context(|| format!("create packaging target {target}"))?;

    for (crates, sibling) in PACKAGING_INVOCATIONS {
        package_sources(&repository, &target, crates, sibling)?;
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
/// # Errors
///
/// Returns an error naming the first missing artefact.
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

/// Run one `cargo package` invocation for `crates`, patching `sibling`.
fn package_sources(
    repository: &Utf8Path,
    target: &Utf8Path,
    crates: &[&str],
    sibling: &str,
) -> Result<()> {
    // The subcommand comes first: `--manifest-path` is a per-command flag.
    let mut args = vec![String::from("package")];
    args.push(String::from("--manifest-path"));
    args.push(repository.join("Cargo.toml").into_string());
    args.push(String::from("--offline"));
    // The worktree is usually dirty while a milestone is being built; the
    // contract is about the sources as they are, not about what is committed.
    args.push(String::from("--allow-dirty"));
    for crate_name in crates {
        args.push(String::from("-p"));
        args.push((*crate_name).to_owned());
    }
    args.push(String::from("--config"));
    args.push(format!(
        "patch.crates-io.{sibling}.path={}",
        toml_string(&sibling_path(repository, sibling)?),
    ));

    let mut command = Command::new(cargo_binary());
    command
        .current_dir(repository)
        .args(&args)
        .env_clear()
        .envs(inherited_toolchain())
        .env("CARGO_TARGET_DIR", target);
    let description = format!("cargo package for {}", crates.join(", "));
    let output = run_with_deadline(command, PACKAGE_DEADLINE, &description)?;
    if !output.status.success() {
        bail!(
            "{description} failed with {}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        );
    }
    Ok(())
}

/// Spell `sibling` as a path relative to the repository, for a Cargo patch.
///
/// Cargo resolves a `--config` patch path against the working directory, so a
/// relative spelling is both correct and stable across checkouts.
fn sibling_path(repository: &Utf8Path, sibling: &str) -> Result<String> {
    let path = repository.join(sibling);
    ensure!(
        path.join("Cargo.toml").is_file(),
        "the sibling patch source {path} is not a crate root",
    );
    Ok(sibling.to_owned())
}

/// A foreign consumer root that can be written and resolved.
///
/// Wraps [`FixtureCargo`] with the consumer root's own files, so a contract
/// reads as "render, write, resolve" rather than repeating the layout each
/// time.
pub(crate) struct ConsumerFixture {
    cargo: FixtureCargo,
}

impl ConsumerFixture {
    /// Create an empty consumer fixture.
    ///
    /// # Errors
    ///
    /// Returns an error when the fixture root cannot be created.
    pub(crate) fn new() -> Result<Self> {
        Ok(Self {
            cargo: FixtureCargo::new()?,
        })
    }

    /// Write a rendered consumer and return its fixture.
    ///
    /// # Errors
    ///
    /// Returns an error when the manifest or library cannot be written.
    pub(crate) fn write_consumer(self, consumer: &super::consumer::Consumer) -> Result<Self> {
        self.cargo
            .write(Utf8Path::new("Cargo.toml"), &consumer.manifest)?;
        self.cargo
            .write(Utf8Path::new("src/lib.rs"), &consumer.library)?;
        Ok(self)
    }

    /// Resolve the consumer root and parse its metadata graph.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo fails or emits an unreadable document.
    pub(crate) fn metadata(&self) -> Result<super::fixture_cargo::Metadata> {
        self.cargo.metadata()
    }

    /// Build and test the consumer root.
    ///
    /// # Errors
    ///
    /// Returns an error carrying both streams when the build or its tests fail.
    pub(crate) fn test(&self) -> Result<()> {
        self.cargo
            .cargo_checked(&["test", "--all-targets", "--all-features"])
            .map(|_stdout| ())
    }
}
