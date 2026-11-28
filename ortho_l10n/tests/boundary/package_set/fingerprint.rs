//! A content fingerprint of everything the packaging recipe reads.
//!
//! The preparation record claims "`cargo package --verify` passed for these
//! sources". That claim is only as good as the binding between the record and
//! the sources, so the fingerprint covers what Cargo packages and resolves:
//! every file of each packaged or patched member (Cargo packages a directory
//! whole unless a manifest narrows it, and none does), the workspace manifest
//! and lockfile, the pinned toolchain file, the Cargo configuration, and the
//! identity of the compiler and Cargo that did the verifying.
//!
//! The walk skips `target` and hidden entries, which Cargo does not package
//! either. It reads bytes through `cap_std`, so a symlink cannot lead it out of
//! the repository.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;
use sha2::{Digest, Sha256};

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

/// Fingerprint the recipe's inputs.
///
/// `recipe` and `toolchain` are folded in so a change to either, not only to a
/// source file, invalidates a record. A missing root file is hashed as absent
/// rather than skipped: a repository that gains a `.cargo/config.toml` has
/// changed how it builds.
///
/// # Errors
///
/// Returns an error when the repository cannot be opened or a file cannot be
/// read.
pub(super) fn inputs(repository: &Utf8Path, recipe: &str, toolchain: &str) -> Result<String> {
    let root = Dir::open_ambient_dir(repository, cap_std::ambient_authority())
        .with_context(|| format!("open repository {repository}"))?;
    let mut hasher = Sha256::new();
    absorb(&mut hasher, "recipe", recipe.as_bytes());
    absorb(&mut hasher, "toolchain", toolchain.as_bytes());
    for path in listed_files(&root)? {
        let bytes = read_if_present(&root, &path)?;
        absorb(
            &mut hasher,
            path.as_str(),
            bytes.as_deref().unwrap_or_default(),
        );
    }
    Ok(hex(&hasher.finalize()))
}

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

/// Whether a directory entry is outside what Cargo packages.
fn is_skipped(name: &str) -> bool {
    name == "target" || name.starts_with('.')
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

/// Fold one labelled value into the hash, unambiguously.
///
/// A decimal length and a colon precede each part, which keeps `("ab", "c")`
/// apart from `("a", "bc")` without depending on a byte order.
fn absorb(hasher: &mut Sha256, label: &str, bytes: &[u8]) {
    for part in [label.as_bytes(), bytes] {
        hasher.update(part.len().to_string().as_bytes());
        hasher.update(b":");
        hasher.update(part);
    }
}
