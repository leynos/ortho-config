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
//! The walk skips `target` and hidden entries, which Cargo does not package
//! either. It reads bytes through `cap_std`, so a symlink cannot lead it out of
//! the repository.

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
    listed_files(&root)?
        .into_iter()
        .map(|path| {
            let digest = read_if_present(&root, &path)?
                .map_or_else(|| ABSENT.to_owned(), |bytes| sha256_hex(&bytes));
            Ok((path.into_string(), digest))
        })
        .collect()
}

/// The digest recorded for a root file that does not exist.
const ABSENT: &str = "absent";

/// Every file the fingerprint covers, in a stable order.
fn listed_files(root: &Dir) -> Result<Vec<Utf8PathBuf>> {
    let mut files: Vec<Utf8PathBuf> = ROOT_FILES.iter().map(Utf8PathBuf::from).collect();
    for member in MEMBER_DIRECTORIES {
        collect(root, Utf8Path::new(member), &mut files)?;
    }
    files.sort();
    Ok(files)
}

/// Add every packageable file beneath `directory` to `files`.
fn collect(root: &Dir, directory: &Utf8Path, files: &mut Vec<Utf8PathBuf>) -> Result<()> {
    for listed in root
        .read_dir(directory)
        .with_context(|| format!("list {directory}"))?
    {
        let entry = listed.with_context(|| format!("read an entry of {directory}"))?;
        let name = entry
            .file_name()
            .with_context(|| format!("name an entry of {directory}"))?;
        if is_skipped(&name) {
            continue;
        }
        let path = directory.join(&name);
        let kind = entry
            .file_type()
            .with_context(|| format!("inspect {path}"))?;
        if kind.is_dir() {
            collect(root, &path, files)?;
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
fn is_skipped(name: &str) -> bool {
    name == "target"
        || name.starts_with('.')
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
