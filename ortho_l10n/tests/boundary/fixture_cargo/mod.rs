//! A private Cargo harness for foreign package roots under `target/`.
//!
//! The boundary contracts must ask Cargo what a *stranger's* package resolves
//! to, and they must ask it from inside `ortho_l10n`'s own integration-test
//! binary. Two properties make that safe:
//!
//! - the fixture root carries its own `[workspace]` table, so Cargo treats it
//!   as a workspace root instead of walking up to the repository workspace;
//! - the child is given a target directory of its own under
//!   [`BOUNDARY_ROOT`], so the nested resolution cannot contend with the parent
//!   build's fingerprints.
//!
//! Child process deadlines, inherited environment, Cargo metadata and manifest
//! inspection each have a focused module: [`command`], [`environment`],
//! [`metadata`] and [`manifest`].

mod command;
pub(crate) mod environment;
pub(crate) mod manifest;
pub(crate) mod metadata;

#[cfg(test)]
#[path = "metadata_proptests.rs"]
mod metadata_proptests;

pub(crate) use command::{run_with_deadline, run_with_deadline_until};
pub(crate) use environment::{cargo_binary, inherited_toolchain};
pub(crate) use manifest::{declared_dependencies, declared_path_dependencies};
pub(crate) use metadata::{DependencyEdge, EdgeKind, Metadata};

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use environment::CARGO_DEADLINE;
use std::process::{Command, Output};
use std::time::Instant;

/// Directory under the repository target tree that owns every fixture root.
///
/// One directory for the whole boundary suite means the packaging step, the
/// nested consumer targets, and the shared package cache live together and can
/// be discarded together.
pub(crate) const BOUNDARY_ROOT: &str = "target/localization-boundary";

/// One disposable foreign package root and its private target directory.
///
/// Dropping the value removes the root. The shared target directory is kept
/// between runs on purpose: a warm package cache and a warm target make two
/// sibling fixtures comparable, and neither is a cold-download claim.
pub(crate) struct FixtureCargo {
    /// The temporary root's drop guard. It is never read: its only job is to
    /// delete the root when the fixture is dropped.
    _tempdir: tempfile::TempDir,
    /// The temporary root as a UTF-8 path, which manifests are rendered from.
    root: Utf8PathBuf,
    /// Target directory shared by the fixtures of one contract family.
    target: Utf8PathBuf,
}

impl FixtureCargo {
    /// Shared host-filtered arguments for this fixture's metadata resolve paths.
    const METADATA_ARGS: [&str; 5] = [
        "metadata",
        "--format-version=1",
        "--all-features",
        "--filter-platform",
        "host-tuple",
    ];

    /// Create an empty fixture root inside the repository target tree.
    ///
    /// # Errors
    ///
    /// Returns an error when the boundary directory cannot be created, when the
    /// temporary root cannot be allocated beneath it, or when `tempfile` reports
    /// that root as a non-UTF-8 path.
    pub(crate) fn new() -> Result<Self> {
        let repository = repository_root()?;
        let boundary = repository.join(BOUNDARY_ROOT);
        Dir::create_ambient_dir_all(&boundary, cap_std::ambient_authority())
            .with_context(|| format!("create boundary root {boundary}"))?;
        let tempdir = tempfile::Builder::new()
            .prefix("fixture-")
            .tempdir_in(&boundary)
            .with_context(|| format!("create fixture root under {boundary}"))?;
        // `tempfile` reports a `std` path and this is the only place the harness
        // converts one. Every manifest the fixtures render names a path, so a
        // non-UTF-8 root is rejected here rather than silently rendered as a
        // lossy string that Cargo would then fail to find.
        let root = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
            .map_err(|path| anyhow::anyhow!("the fixture root {} is not UTF-8", path.display()))?;
        Ok(Self {
            _tempdir: tempdir,
            root,
            target: boundary.join("cargo-target"),
        })
    }

    /// Path of this fixture's package root.
    pub(crate) fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// Write `contents` to `relative_path` beneath the fixture root.
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory or the file cannot be
    /// written.
    pub(crate) fn write(&self, relative_path: &Utf8Path, contents: &str) -> Result<()> {
        let directory = Dir::open_ambient_dir(self.root(), cap_std::ambient_authority())
            .with_context(|| format!("open fixture root {}", self.root()))?;
        if let Some(parent) = relative_path.parent() {
            directory
                .create_dir_all(parent)
                .with_context(|| format!("create {parent} in fixture root"))?;
        }
        directory
            .write(relative_path, contents)
            .with_context(|| format!("write {relative_path} in fixture root"))
    }

    /// Run `cargo` with `args`, returning its captured output.
    ///
    /// The child receives the allow-listed toolchain environment, a
    /// `CARGO_TARGET_DIR` pointed at this fixture's private target, and
    /// `--offline`. Offline is not an optimization: a boundary contract has to
    /// prove resolution from the shared package cache, and a network fetch
    /// would make it prove something about crates.io instead.
    ///
    /// The child is deadline-bounded. A nested `cargo metadata` that hangs on
    /// the package-cache lock would otherwise consume the whole test allowance
    /// and hide which contract was waiting.
    ///
    /// # Errors
    ///
    /// Returns an error when the Cargo process cannot be spawned or exceeds
    /// [`CARGO_DEADLINE`].
    pub(crate) fn cargo(&self, args: &[&str]) -> Result<Output> {
        self.cargo_with_deadline(args, None)
    }

    /// Run `cargo` within an enclosing test sequence's deadline.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo cannot be spawned or the command or its
    /// enclosing sequence exceeds its deadline.
    pub(crate) fn cargo_until(&self, args: &[&str], sequence_deadline: Instant) -> Result<Output> {
        self.cargo_with_deadline(args, Some(sequence_deadline))
    }

    fn cargo_with_deadline(
        &self,
        args: &[&str],
        sequence_deadline: Option<Instant>,
    ) -> Result<Output> {
        let mut command = Command::new(cargo_binary());
        // The subcommand has to come first: `--manifest-path` and `--offline`
        // are per-command flags, not global ones, so `cargo --manifest-path X
        // metadata` is a usage error.
        command
            .current_dir(self.root.as_std_path())
            .args(args)
            .arg("--manifest-path")
            .arg(self.root.join("Cargo.toml").as_std_path())
            .arg("--offline")
            .env_clear()
            .envs(inherited_toolchain())
            .env("CARGO_TARGET_DIR", &self.target);
        let description = format!("cargo {}", args.join(" "));
        match sequence_deadline {
            Some(deadline) => {
                run_with_deadline_until(command, CARGO_DEADLINE, deadline, &description)
            }
            None => run_with_deadline(command, CARGO_DEADLINE, &description),
        }
    }

    /// Run `cargo` and require a successful exit status.
    ///
    /// # Errors
    ///
    /// Returns an error carrying both streams when Cargo exits non-zero.
    pub(crate) fn cargo_checked(&self, args: &[&str]) -> Result<String> {
        let output = self.cargo(args)?;
        checked_stdout(&output, &format!("cargo {}", args.join(" ")))
    }

    /// Run `cargo` within an enclosing test sequence's deadline and require success.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo fails or the command or its enclosing
    /// sequence exceeds its deadline.
    pub(crate) fn cargo_checked_until(
        &self,
        args: &[&str],
        sequence_deadline: Instant,
    ) -> Result<String> {
        let output = self.cargo_until(args, sequence_deadline)?;
        checked_stdout(&output, &format!("cargo {}", args.join(" ")))
    }

    /// Resolve `cargo metadata` for the fixture root, as the host sees it.
    ///
    /// Every contract that resolves must go through here rather than building
    /// its own argument list, so the host narrowing cannot be present in one
    /// call site and missing in another. It was missing in the negative
    /// controls, which is where the Windows failure appeared.
    ///
    /// The resolve is narrowed to the host, which is what makes it portable.
    /// Under `--offline` an unfiltered resolve must download every platform's
    /// dependencies, including crates the host never builds and a warm runner
    /// cache therefore does not hold; `figment` gates `atomic` on
    /// `target_pointer_width`, so a 64-bit runner never fetched it and the
    /// Windows leg failed on exactly that. `host-tuple` is Cargo's own name for
    /// the running host. Narrowing costs no coverage: these contracts read a
    /// *host* build graph, and the platform-independent half - that no manifest
    /// *declares* a forbidden edge, target tables included - is asserted over
    /// the manifest text by [`declared_dependencies`], on every platform.
    ///
    /// The raw output is returned as well as the parsed document, because the
    /// negative controls must inspect a *failed* resolve's diagnostic, which
    /// has no document to parse.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo cannot be spawned or exceeds the deadline.
    /// A non-zero exit is not an error here; the caller decides.
    pub(crate) fn resolve(&self) -> Result<Output> {
        self.cargo(&Self::METADATA_ARGS)
    }

    /// Resolve metadata while respecting an enclosing test sequence's deadline.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo cannot be spawned or either deadline expires.
    pub(crate) fn resolve_until(&self, sequence_deadline: Instant) -> Result<Output> {
        self.cargo_until(&Self::METADATA_ARGS, sequence_deadline)
    }

    /// Resolve and parse the fixture root, requiring Cargo to succeed.
    ///
    /// Spawns Cargo, so it is named for the command rather than for the
    /// document it returns; [`Self::resolve`] is the same call without the
    /// success requirement.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo fails or emits something other than the
    /// single JSON document the contract expects.
    pub(crate) fn resolve_metadata(&self) -> Result<Metadata> {
        let output = self.resolve()?;
        Metadata::parse(&checked_stdout(&output, "cargo metadata")?)
    }

    /// Resolve and parse metadata within an enclosing test sequence's deadline.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo fails, metadata is unreadable, or either
    /// deadline expires.
    pub(crate) fn resolve_metadata_until(&self, sequence_deadline: Instant) -> Result<Metadata> {
        let output = self.resolve_until(sequence_deadline)?;
        Metadata::parse(&checked_stdout(&output, "cargo metadata")?)
    }
}

/// Unwrap a successful run's standard output, or report both streams.
///
/// The two callers differ only in whether they mean "every command this
/// fixture runs" or "the metadata resolve in particular", so the label is a
/// parameter and the failure handling has exactly one implementation. A
/// duplicated copy here would let the two drift: whichever one a future edit
/// missed would report a truncated failure, and the negative controls rely on
/// a failing resolve's diagnostic being reported in full.
///
/// # Errors
///
/// Returns an error naming `label` when the child exited non-zero.
fn checked_stdout(output: &Output, label: &str) -> Result<String> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    ensure!(
        output.status.success(),
        "{label} failed with {}\nstdout:\n{stdout}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(stdout)
}

/// Path of the repository root, derived from this crate's manifest directory.
///
/// # Errors
///
/// Returns an error when `CARGO_MANIFEST_DIR` has no parent.
pub(crate) fn repository_root() -> Result<Utf8PathBuf> {
    let manifest = Utf8Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map(Utf8Path::to_path_buf)
        .context("locate repository root from ortho_l10n manifest directory")
}

/// Describe the edges that were found, for a missing-or-forbidden message.
pub(crate) fn describe_edges(edges: &[DependencyEdge]) -> String {
    edges
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Render a plain string as a TOML basic string.
pub(crate) fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_owned()).to_string()
}

/// Canonicalize a workspace member's crate root, for a manifest `path` key.
///
/// The spelling is absolute because a rendered consumer lives under `target/`,
/// not beside the members: Cargo resolves a relative path dependency against
/// the package that declares it, so a relative spelling would look for
/// `ortho_l10n` inside the fixture root and find nothing.
///
/// # Errors
///
/// Returns an error when the directory does not exist or holds no manifest.
pub(crate) fn absolute_member(repository: &Utf8Path, member: &Utf8Path) -> Result<String> {
    let crate_root = repository.join(member);
    let absolute = crate_root
        .canonicalize_utf8()
        .with_context(|| format!("canonicalize workspace member {crate_root}"))?;
    ensure!(
        absolute.join("Cargo.toml").is_file(),
        "{absolute} is not a crate root",
    );
    Ok(absolute.into_string())
}
