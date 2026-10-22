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
//! The environment a child may inherit is a separate module, [`environment`],
//! because it is the part a reviewer most needs to be able to check at a
//! glance. The shape of Cargo's metadata document is a separate module too,
//! [`metadata`], because it is an external tool contract rather than fixture
//! plumbing.

pub(crate) mod environment;
pub(crate) mod metadata;

pub(crate) use environment::{cargo_binary, inherited_toolchain};
pub(crate) use metadata::{DependencyEdge, EdgeKind, Metadata};

use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use environment::CARGO_DEADLINE;
use std::collections::BTreeSet;
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt as _;

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
        run_with_deadline(
            command,
            CARGO_DEADLINE,
            &format!("cargo {}", args.join(" ")),
        )
    }

    /// Run `cargo` and require a successful exit status.
    ///
    /// # Errors
    ///
    /// Returns an error carrying both streams when Cargo exits non-zero.
    pub(crate) fn cargo_checked(&self, args: &[&str]) -> Result<String> {
        let output = self.cargo(args)?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if !output.status.success() {
            bail!(
                "cargo {} failed with {}\nstdout:\n{}\nstderr:\n{}",
                args.join(" "),
                output.status,
                stdout,
                String::from_utf8_lossy(&output.stderr),
            );
        }
        Ok(stdout)
    }

    /// Resolve `cargo metadata` for the fixture root and parse it as JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when Cargo fails or emits something other than the
    /// single JSON document the contract expects.
    pub(crate) fn metadata(&self) -> Result<Metadata> {
        let stdout = self.cargo_checked(&["metadata", "--format-version=1", "--all-features"])?;
        Metadata::parse(&stdout)
    }
}

/// Run `command` to completion or the deadline, whichever comes first.
///
/// Both streams are drained on their own threads. A nested Cargo build prints
/// far more than a pipe buffer holds, and a runner that waits for the exit
/// status before reading the pipes therefore deadlocks: the child blocks
/// writing, the parent blocks waiting, and the deadline reports a hang that is
/// really the harness's own. Draining concurrently removes that failure mode,
/// so a reported deadline means the child itself made no progress.
///
/// # Errors
///
/// Returns an error when the child cannot be spawned or collected, or when it
/// is still running at the deadline. A timed-out child is killed before the
/// error is returned, so the test process is not left with an orphan holding
/// the package-cache lock.
pub(crate) fn run_with_deadline(
    mut command: Command,
    deadline: Duration,
    description: &str,
) -> Result<Output> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawn {description}"))?;
    let stdout_reader = drain(
        child.stdout.take(),
        &format!("read stdout of {description}"),
    )?;
    let stderr_reader = drain(
        child.stderr.take(),
        &format!("read stderr of {description}"),
    )?;

    let waited = child
        .wait_timeout(deadline)
        .with_context(|| format!("wait for {description}"))?;
    let Some(status) = waited else {
        // Reap the child: a killed process releases its package-cache lock and
        // its compile-admission slot, which a leaked one would hold.
        let _killed = child.kill();
        let _reaped = child.wait();
        bail!(
            "{description} exceeded the {} second deadline",
            deadline.as_secs(),
        );
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("the stdout reader of {description} panicked"))?
        .with_context(|| format!("read stdout of {description}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("the stderr reader of {description} panicked"))?
        .with_context(|| format!("read stderr of {description}"))?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Start a thread that reads `handle` to end of file.
fn drain<R: std::io::Read + Send + 'static>(
    handle: Option<R>,
    description: &str,
) -> Result<std::thread::JoinHandle<std::io::Result<Vec<u8>>>> {
    let mut stream = handle.with_context(|| format!("capture {description}"))?;
    std::thread::Builder::new()
        .name(description.to_owned())
        .spawn(move || {
            let mut buffer = Vec::new();
            stream.read_to_end(&mut buffer)?;
            Ok(buffer)
        })
        .with_context(|| format!("start the reader for {description}"))
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

/// Assert that a captured Cargo failure mentions `needle`.
///
/// A negative control is only evidence if its failure names the offending
/// dependency: an unrelated resolution error would satisfy a bare
/// "did not succeed" check while proving nothing.
///
/// # Errors
///
/// Returns an error when Cargo succeeded, or when neither stream mentions the
/// expected text.
pub(crate) fn ensure_failure_names(output: &Output, needle: &str) -> Result<()> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        !output.status.success(),
        "the negative control succeeded; its forbidden dependency was accepted\nstdout:\n{stdout}"
    );
    ensure!(
        stdout.contains(needle) || stderr.contains(needle),
        concat!(
            "the negative control failed for a reason that does not name {needle:?}, ",
            "so it cannot show the injected edge was rejected\nstdout:\n{}\nstderr:\n{}"
        ),
        stdout,
        stderr,
        needle = needle,
    );
    Ok(())
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

/// Every dependency edge a manifest *declares*, as its tables name them.
///
/// This reads the packaged manifest as data rather than as text, so a crate
/// that a comment or a feature table merely mentions is not mistaken for a
/// declared edge. Target-conditional tables are walked too: an edge declared
/// for one platform is still an edge a stranger's platform may resolve.
///
/// The kind is the table the edge sits under, which is what distinguishes a
/// published edge from a test-only one. A dependency that Cargo inherited into
/// a platform table is reported once per table, matching how it was declared.
///
/// # Errors
///
/// Returns an error when `manifest` is not a TOML document.
pub(crate) fn declared_dependencies(manifest: &str) -> Result<Vec<DependencyEdge>> {
    let document: toml::Table =
        toml::from_str(manifest).context("parse a packaged manifest as a TOML document")?;
    let mut edges = BTreeSet::new();
    collect_declared_dependencies(&document, &mut edges);
    Ok(edges
        .into_iter()
        .map(|(package, kind)| DependencyEdge { package, kind })
        .collect())
}

/// The manifest tables whose keys are dependency names, with their kinds.
const DEPENDENCY_TABLES: [(&str, EdgeKind); 3] = [
    ("dependencies", EdgeKind::Normal),
    ("dev-dependencies", EdgeKind::Dev),
    ("build-dependencies", EdgeKind::Build),
];

/// The keys of one dependency table, or nothing when it is not declared.
fn dependency_names<'a>(table: &'a toml::Table, heading: &str) -> impl Iterator<Item = &'a String> {
    table
        .get(heading)
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::keys)
}

/// The platform-conditional tables a manifest nests its edges in.
fn target_tables(table: &toml::Table) -> impl Iterator<Item = &toml::Table> {
    table
        .get("target")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(toml::Table::values)
        .filter_map(toml::Value::as_table)
}

/// Walk `table`, collecting declared edges from it and its target sections.
fn collect_declared_dependencies(table: &toml::Table, edges: &mut BTreeSet<(String, EdgeKind)>) {
    for (heading, kind) in DEPENDENCY_TABLES {
        for name in dependency_names(table, heading) {
            edges.insert((name.clone(), kind));
        }
    }
    for section in target_tables(table) {
        collect_declared_dependencies(section, edges);
    }
}
