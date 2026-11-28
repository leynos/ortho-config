//! The preparation record: proof that packaging ran, outside the timed run.
//!
//! The timed boundary contracts used to run both verified `cargo package`
//! invocations inside their own allowance. `cargo package --verify` unpacks the
//! crate and compiles the packaged closure every time, so a warm target
//! directory does not shorten it, and on a shared host the compile-admission
//! queue alone outlasted the allowance (#700, #755).
//!
//! Preparation moves that work out. `make boundary-prepare` runs the same
//! invocations, with the same patches and without `--no-verify`, under no
//! per-test deadline, and writes a record once both have succeeded. A timed
//! contract then *checks* the record and reads the unpacked roots; it does not
//! package. The record is evidence, not a switch: it names what was verified,
//! binds that to a [digest per file](super::fingerprint) of the sources and the
//! lockfile, to the toolchain and to the recipe, and carries the digest of each
//! archive, so the roots a contract reads are the ones that passed
//! verification. A stale record names the files that changed.
//!
//! A missing or stale record is a refusal that names `make boundary-prepare`,
//! not a timeout, and nothing here reaches the network.

use super::super::fixture_cargo::{cargo_binary, inherited_toolchain};
use super::fingerprint::{file_digests, sha256_hex, tree_digests};
use super::{PACKAGED_CRATES, PACKAGING_INVOCATIONS};
use anyhow::{Context, Result, bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::process::Command;
use std::sync::OnceLock;

/// Name of the record, beside the packaging target it describes.
const RECORD_FILE: &str = "prepared.json";

/// Bump when a field of the record, or what the fingerprint covers, changes.
const RECORD_FORMAT: u32 = 2;

/// What `cargo package` is asked to do, spelled out for the fingerprint.
///
/// A change to the crate lists or the patches changes the recipe, and a record
/// written under the old one must not satisfy the new one.
fn recipe() -> String {
    let invocations = PACKAGING_INVOCATIONS
        .iter()
        .map(|invocation| {
            let patches = invocation
                .patched
                .iter()
                .map(|(package, directory)| format!("{package}={directory}"))
                .collect::<Vec<_>>();
            format!(
                "package [{}] patching [{}]",
                invocation.crates.join(","),
                patches.join(","),
            )
        })
        .collect::<Vec<_>>();
    format!(
        "format {RECORD_FORMAT}; features --all-features; verify on; {}",
        invocations.join("; "),
    )
}

/// What the preparation step recorded.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    /// The record's own layout version.
    format: u32,
    /// The release version every packaged crate shares.
    version: String,
    /// What `cargo package` was asked to do.
    recipe: String,
    /// The compiler and Cargo that verified the packages.
    toolchain: String,
    /// Digest of every source, manifest and lock file the recipe read.
    files: BTreeMap<String, String>,
    /// One entry per `cargo package --verify` invocation that exited zero.
    verified: Vec<String>,
    /// SHA-256 of each `.crate` archive and every file of each unpacked root.
    artefacts: BTreeMap<String, String>,
}

/// Path of the record for a packaging `target`.
fn record_path(target: &Utf8Path) -> Utf8PathBuf {
    target.join(RECORD_FILE)
}

/// Description of each verified invocation, as the packaging step names it.
fn invocation_descriptions() -> Vec<String> {
    PACKAGING_INVOCATIONS
        .iter()
        .map(|invocation| format!("cargo package for {}", invocation.crates.join(", ")))
        .collect()
}

/// Digest of every archive, and of every file of every unpacked root, beneath
/// `target/package`.
///
/// The archives are what verification compiled; the unpacked roots are what the
/// contracts read and what the consumer builds compile, since a consumer's
/// `[patch]` points at them rather than at the archives. Digesting both ties
/// the roots a contract reads, sources and resources included, to the archives
/// that passed.
fn artefact_digests(target: &Utf8Path) -> Result<BTreeMap<String, String>> {
    let directory = Dir::open_ambient_dir(target.join("package"), cap_std::ambient_authority())
        .with_context(|| format!("open {target}/package"))?;
    let mut digests = BTreeMap::new();
    for (name, version) in PACKAGED_CRATES {
        let archive = format!("{name}-{version}.crate");
        let bytes = directory
            .read(&archive)
            .with_context(|| format!("read packaged archive {archive}"))?;
        digests.insert(archive, sha256_hex(&bytes));
        let root = Utf8PathBuf::from(format!("{name}-{version}"));
        digests.extend(tree_digests(&directory, &root)?);
    }
    Ok(digests)
}

/// What the sources, lockfile and toolchain were when preparation began.
///
/// Captured before packaging starts and published after it ends, so a source
/// edited while `cargo package` runs is recorded as it was *before*: the
/// archives may hold the edit, the record does not, and the first check
/// refuses it as stale instead of vouching for an unverified tree.
#[derive(Debug)]
pub(super) struct Snapshot {
    /// The compiler and Cargo in force.
    toolchain: String,
    /// Digest of every source, manifest and lock file the recipe reads.
    files: BTreeMap<String, String>,
}

impl Snapshot {
    /// Digest the sources now.
    ///
    /// # Errors
    ///
    /// Returns an error when a source cannot be read.
    pub(super) fn capture(repository: &Utf8Path, toolchain: &str) -> Result<Self> {
        Ok(Self {
            toolchain: toolchain.to_owned(),
            files: file_digests(repository)?,
        })
    }
}

/// Discard the record before packaging starts.
///
/// A record that survives a half-finished run would vouch for roots that are
/// being replaced, so preparation removes it first and writes it last.
///
/// # Errors
///
/// Returns an error when an existing record cannot be removed.
pub(super) fn invalidate(target: &Utf8Path) -> Result<()> {
    let Ok(directory) = Dir::open_ambient_dir(target, cap_std::ambient_authority()) else {
        return Ok(());
    };
    if directory.try_exists(RECORD_FILE).unwrap_or(false) {
        directory
            .remove_file(RECORD_FILE)
            .with_context(|| format!("discard the stale record {}", record_path(target)))?;
    }
    Ok(())
}

/// Publish the record for a preparation that has just succeeded.
///
/// Call it last: after both packaging invocations and every consumer build, so
/// that a preparation that stopped early leaves no record. `snapshot` was taken
/// before packaging began. The record is written to a temporary name of this
/// process's own and renamed, so a reader never sees half of it and two runs
/// in one checkout do not write the same file.
///
/// # Errors
///
/// Returns an error when the archives or unpacked roots, or the record, cannot
/// be read or written.
pub(super) fn write(snapshot: Snapshot, target: &Utf8Path) -> Result<()> {
    let record = Record {
        format: RECORD_FORMAT,
        version: super::PACKAGED_VERSION.to_owned(),
        recipe: recipe(),
        toolchain: snapshot.toolchain,
        files: snapshot.files,
        verified: invocation_descriptions(),
        artefacts: artefact_digests(target)?,
    };
    let text = serde_json::to_string_pretty(&record).context("render the preparation record")?;
    let directory = Dir::open_ambient_dir(target, cap_std::ambient_authority())
        .with_context(|| format!("open packaging target {target}"))?;
    let temporary = format!("{RECORD_FILE}.{}.partial", std::process::id());
    directory
        .write(&temporary, text)
        .with_context(|| format!("write {target}/{temporary}"))?;
    directory
        .rename(&temporary, &directory, RECORD_FILE)
        .with_context(|| format!("publish {}", record_path(target)))
}

/// Refuse unless the record in `target` vouches for the current sources.
///
/// This is the whole of what a timed contract does about packaging: read the
/// record, recompute the fingerprint, and compare the archive digests. It
/// spawns nothing and touches no network.
///
/// # Errors
///
/// Returns an error naming `make boundary-prepare` when the record is missing,
/// unreadable, or no longer matches the sources, the toolchain, or the
/// archives it describes.
pub(super) fn check(repository: &Utf8Path, target: &Utf8Path, toolchain: &str) -> Result<()> {
    let path = record_path(target);
    let text = read_record(target).with_context(|| {
        format!(
            "the boundary suite has not been prepared: no record at {path}; \
             run `make boundary-prepare`"
        )
    })?;
    let record: Record = serde_json::from_str(&text).with_context(|| {
        format!("the preparation record {path} is unreadable; run `make boundary-prepare`")
    })?;
    if let Err(reason) = matches_current(&record, repository, target, toolchain) {
        bail!("the preparation record {path} is stale: {reason:#}; run `make boundary-prepare`");
    }
    Ok(())
}

/// The identity of the Cargo and compiler a nested child would run, once per process.
///
/// Version probes, not builds: neither reads a manifest nor takes the package
/// cache lock. The child environment is the same closed one the packaging
/// invocations get, so the probe sees the toolchain they saw.
///
/// # Errors
///
/// Returns an error when either probe cannot run or exits non-zero.
pub(super) fn toolchain_identity(repository: &Utf8Path) -> Result<String> {
    static IDENTITY: OnceLock<Result<String, String>> = OnceLock::new();
    IDENTITY
        .get_or_init(|| probe_toolchain(repository).map_err(|error| format!("{error:#}")))
        .clone()
        .map_err(|error| anyhow::anyhow!("{error}"))
}

/// Run `cargo -V` and `rustc -vV` and join what they print.
fn probe_toolchain(repository: &Utf8Path) -> Result<String> {
    let cargo = probe(repository, &cargo_binary(), "-V")?;
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let compiler = probe(repository, &rustc, "-vV")?;
    Ok(format!("{cargo}\n{compiler}"))
}

/// Run one version probe and return its trimmed standard output.
fn probe(repository: &Utf8Path, program: &OsStr, flag: &str) -> Result<String> {
    let output = Command::new(program)
        .arg(flag)
        .current_dir(repository)
        .env_clear()
        .envs(inherited_toolchain())
        .output()
        .with_context(|| format!("run {} {flag}", program.to_string_lossy()))?;
    ensure!(
        output.status.success(),
        "{} {flag} exited with {}",
        program.to_string_lossy(),
        output.status,
    );
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Read the record's text.
fn read_record(target: &Utf8Path) -> Result<String> {
    let directory = Dir::open_ambient_dir(target, cap_std::ambient_authority())?;
    Ok(directory.read_to_string(RECORD_FILE)?)
}

/// Compare a record with the sources and archives as they are now.
fn matches_current(
    record: &Record,
    repository: &Utf8Path,
    target: &Utf8Path,
    toolchain: &str,
) -> Result<()> {
    ensure!(
        record.format == RECORD_FORMAT,
        "it was written by record format {}, this suite reads {RECORD_FORMAT}",
        record.format,
    );
    ensure!(
        record.version == super::PACKAGED_VERSION,
        "it describes version {}, the sources are {}",
        record.version,
        super::PACKAGED_VERSION,
    );
    ensure!(
        record.verified == invocation_descriptions(),
        "it does not record every verified packaging invocation",
    );
    ensure!(
        record.recipe == recipe(),
        "the packaging recipe changed since it was written",
    );
    ensure!(
        record.toolchain == toolchain,
        "the toolchain differs: it was written under {:?} and this process sees {toolchain:?}",
        record.toolchain,
    );
    let changes = describe_changes(&record.files, &file_digests(repository)?);
    ensure!(
        changes.is_empty(),
        "files the packaging reads changed since it was written ({changes})",
    );
    let artefact_changes = describe_changes(&record.artefacts, &artefact_digests(target)?);
    ensure!(
        artefact_changes.is_empty(),
        "the packaged archives or unpacked roots differ from those that passed verification \
         ({artefact_changes})",
    );
    Ok(())
}

/// How many changed paths a stale-record message names before it summarizes.
const NAMED_CHANGES: usize = 8;

/// Say which files differ between a record and the sources now, or nothing.
///
/// A stale record is only useful if it says what made it stale: a source the
/// developer edited, and a file some other process left in a packaged member,
/// call for different fixes.
fn describe_changes(
    recorded: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
) -> String {
    let mut changes: Vec<(&str, &str)> = Vec::new();
    for (path, digest) in current {
        match recorded.get(path) {
            None => changes.push(("added", path)),
            Some(before) if before != digest => changes.push(("changed", path)),
            Some(_unchanged) => {}
        }
    }
    changes.extend(
        recorded
            .keys()
            .filter(|path| !current.contains_key(*path))
            .map(|path| ("removed", path.as_str())),
    );
    let (named, rest) = changes.split_at(changes.len().min(NAMED_CHANGES));
    let mut parts: Vec<String> = named
        .iter()
        .map(|(verb, path)| format!("{verb} {path}"))
        .collect();
    if !rest.is_empty() {
        parts.push(format!(
            "and {} more: {}",
            rest.len(),
            summarize_by_pattern(rest)
        ));
    }
    parts.join(", ")
}

/// How many kinds of change the summary of the unnamed ones lists.
const SUMMARIZED_KINDS: usize = 5;

/// Count `changes` by what they did and where, as `3 x added dir/*.ext`.
///
/// A stale record caused by a process that wrote hundreds of files names its
/// cause once rather than eight paths of it.
fn summarize_by_pattern(changes: &[(&str, &str)]) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (verb, path) in changes {
        let location = Utf8Path::new(path);
        let directory = location.parent().map_or("", Utf8Path::as_str);
        let pattern = location.extension().map_or_else(
            || (*path).to_owned(),
            |extension| format!("{directory}/*.{extension}"),
        );
        *counts.entry(format!("{verb} {pattern}")).or_default() += 1;
    }
    let mut kinds: Vec<(usize, String)> = counts.into_iter().map(|(kind, n)| (n, kind)).collect();
    kinds.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    kinds
        .iter()
        .take(SUMMARIZED_KINDS)
        .map(|(count, kind)| format!("{count} x {kind}"))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
#[path = "prepared_tests.rs"]
mod prepared_tests;
