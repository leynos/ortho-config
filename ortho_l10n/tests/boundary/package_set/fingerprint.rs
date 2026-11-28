//! A per-file digest of everything the packaging recipe reads.
//!
//! The preparation record claims "`cargo package --verify` passed for these
//! sources". That claim is only as good as the binding between the record and
//! the sources, so the digests cover what Cargo packages and resolves: every
//! file of each packaged or patched member (Cargo packages a directory whole
//! unless a manifest narrows it, and none does), the workspace manifest and
//! lockfile, the pinned toolchain file, and the Cargo configuration. One digest
//! per file, not one over all of them, so a stale record can name what changed.
//!
//! The walk skips `target`, `.git` and coverage profile data, which Cargo does
//! not package or which is run output, and covers hidden files, which Cargo
//! does package. It reads bytes through `cap_std`, so a symlink cannot lead it
//! out of the repository.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

/// Workspace members the packaging recipe packages or patches, by directory.
const MEMBER_DIRECTORIES: [&str; 4] = [
    "ortho_l10n",
    "ortho_config",
    "ortho_config_macros",
    "test_helpers",
];

/// Files at the repository root that decide how the members resolve and build.
const ROOT_FILES: [&str; 4] = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo/config.toml",
];

/// Lower-case hexadecimal rendering of a digest.
pub(super) fn hex(digest: &[u8]) -> String {
    digest
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
        .collect()
}

/// SHA-256 of `bytes`, in hexadecimal.
pub(super) fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// The digest of every file the recipe reads, keyed by repository-relative path.
///
/// A file per entry rather than one hash over all of them, so a stale record can
/// name the files that changed. A missing root file is recorded as absent
/// rather than skipped: a repository that gains a `.cargo/config.toml` has
/// changed how it builds.
///
/// # Errors
///
/// Returns an error when the repository cannot be opened or a file cannot be
/// read.
pub(super) fn file_digests(repository: &Utf8Path) -> Result<BTreeMap<String, String>> {
    let root = Dir::open_ambient_dir(repository, cap_std::ambient_authority())
        .with_context(|| format!("open repository {repository}"))?;
    digest_files(&root, listed_files(&root)?)
}

/// The digest of every file beneath `directory` of `root`, packaged or not.
///
/// For the unpacked package roots, which the consumer builds read directly:
/// whatever is in them is what gets compiled, so none of it is skipped.
///
/// # Errors
///
/// Returns an error when a directory cannot be listed or a file cannot be read.
pub(super) fn tree_digests(root: &Dir, directory: &Utf8Path) -> Result<BTreeMap<String, String>> {
    let mut files = Vec::new();
    collect(root, directory, &mut files, |_name| false)?;
    files.sort();
    digest_files(root, files)
}

/// Digest each of `files`, keyed by path spelled with `/` on every platform.
fn digest_files(root: &Dir, files: Vec<Utf8PathBuf>) -> Result<BTreeMap<String, String>> {
    files
        .into_iter()
        .map(|path| {
            let digest = read_if_present(root, &path)?
                .map_or_else(|| ABSENT.to_owned(), |bytes| sha256_hex(&bytes));
            // One spelling on every platform: a record names paths with `/`,
            // and Windows joins them with `\`.
            Ok((path.as_str().replace('\\', "/"), digest))
        })
        .collect()
}

/// The digest recorded for a root file that does not exist.
const ABSENT: &str = "absent";

/// Every file the fingerprint covers, in a stable order.
fn listed_files(root: &Dir) -> Result<Vec<Utf8PathBuf>> {
    let mut files: Vec<Utf8PathBuf> = ROOT_FILES.iter().map(Utf8PathBuf::from).collect();
    for member in MEMBER_DIRECTORIES {
        collect(root, Utf8Path::new(member), &mut files, is_skipped)?;
    }
    files.sort();
    Ok(files)
}

/// Add every file beneath `directory` that `skip` does not exclude to `files`.
fn collect(
    root: &Dir,
    directory: &Utf8Path,
    files: &mut Vec<Utf8PathBuf>,
    skip: fn(&str) -> bool,
) -> Result<()> {
    for listed in root
        .read_dir(directory)
        .with_context(|| format!("list {directory}"))?
    {
        let entry = listed.with_context(|| format!("read an entry of {directory}"))?;
        let name = entry
            .file_name()
            .with_context(|| format!("name an entry of {directory}"))?;
        if skip(&name) {
            continue;
        }
        let path = directory.join(&name);
        let kind = entry
            .file_type()
            .with_context(|| format!("inspect {path}"))?;
        if kind.is_dir() {
            collect(root, &path, files, skip)?;
        } else if kind.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

/// Suffixes of the profile data an instrumented test binary writes.
///
/// A coverage run leaves `default_*.profraw` files in the working directory of
/// the tests it runs, which is a member directory. They appear while the suite
/// is running, after the preparation step has written its record, and they are
/// run output, not a source: a stale-record failure on the first of them
/// (found on CI) says nothing about what was verified.
const COVERAGE_ARTEFACT_SUFFIXES: [&str; 2] = [".profraw", ".profdata"];

/// Whether a directory entry is outside what Cargo packages, or is run output.
///
/// Hidden entries are covered: Cargo packages a tracked hidden file, and a
/// source such as `src/.catalogue.ftl` that a build embeds is still a source.
/// Only `.git`, which is version-control state, is left out.
fn is_skipped(name: &str) -> bool {
    name == "target"
        || name == ".git"
        || COVERAGE_ARTEFACT_SUFFIXES
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

/// Read a file, or `None` when it does not exist.
fn read_if_present(root: &Dir, path: &Utf8Path) -> Result<Option<Vec<u8>>> {
    if !root.try_exists(path).unwrap_or(false) {
        return Ok(None);
    }
    root.read(path)
        .map(Some)
        .with_context(|| format!("read {path}"))
}
