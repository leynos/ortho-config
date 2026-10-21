//! Cache key and fingerprint helpers for `cargo-orthohelp`.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use sha2::{Digest, Sha256};
use std::io::Read;

use crate::error::OrthohelpError;
use crate::hex::to_lower_hex;

/// Number of digest bytes rendered into the cache-directory name.
///
/// The digest names a directory under the workspace target directory, and from
/// cargo 1.100 every artefact of a build lives under
/// `target/debug/build/<crate>/<hash>/out/` rather than `target/debug/deps/`.
/// That layout lengthens every path the linker is handed, and Windows still
/// applies a 260-character `MAX_PATH` to the file names `link.exe` opens, so a
/// 64-digit directory name can push an otherwise ordinary build over the
/// limit. `orthohelp_policy_warn_fixture` reached 262 characters on a hosted
/// runner and failed with `LNK1104`.
///
/// Sixteen bytes (128 bits, 32 digits) halves the directory's contribution.
/// It remains far wider than the input space: a collision needs two distinct
/// cache inputs whose digests agree in 128 bits, and the consequences of one
/// are a stale IR reuse rather than a corrupt result. [`CacheKey::hash`] is
/// still computed over the full SHA-256 digest, so the truncation happens at
/// rendering time and cannot mask a defect in how the inputs are combined.
pub(crate) const CACHE_DIRECTORY_DIGEST_BYTES: usize = 16;

/// Cache key inputs for the bridge IR.
#[derive(Debug, Clone)]
pub struct CacheKey {
    /// SHA-256 fingerprint of package files that can affect the generated IR.
    pub(crate) fingerprint: String,
    /// Qualified root configuration type, separating IR for distinct structs.
    pub(crate) root_type: String,
    /// Generator version, invalidating entries when the producing tool changes.
    pub(crate) tool_version: String,
    /// IR schema version, invalidating entries after serialized shape changes.
    pub(crate) ir_version: String,
    /// Optional workspace lockfile digest, distinct from absence in the hash.
    pub(crate) lockfile_hash: Option<String>,
}

impl CacheKey {
    /// Hashes the cache inputs into a stable directory name.
    ///
    /// The returned identifier is 32 lowercase hexadecimal digits, the leading
    /// [`CACHE_DIRECTORY_DIGEST_BYTES`] bytes of the SHA-256 digest over every
    /// input. It is a name, not a security boundary: see that constant for why
    /// the digest is truncated and what a collision would cost.
    ///
    /// # Examples
    ///
    /// Two keys differing in any single input hash to different names, so an
    /// edited package or a changed generator version never reuses the previous
    /// entry. The example is stated in prose rather than as a doctest because
    /// the inputs are crate-private.
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.fingerprint.as_bytes());
        hasher.update(self.root_type.as_bytes());
        hasher.update(self.tool_version.as_bytes());
        hasher.update(self.ir_version.as_bytes());
        hasher.update(b"lockfile:");
        match &self.lockfile_hash {
            Some(hash) => hasher.update(hash.as_bytes()),
            None => hasher.update(b"none"),
        }
        // Copy the digest prefix into a fixed-size array rather than slicing
        // it: the workspace denies `clippy::indexing_slicing`, and a total
        // copy keeps the rendering free of a panic path. `Sha256` always
        // yields 32 bytes, so the copy is complete in practice; the const
        // assertion below makes that a compile-time guarantee rather than a
        // comment.
        let mut prefix = [0_u8; CACHE_DIRECTORY_DIGEST_BYTES];
        for (slot, byte) in prefix.iter_mut().zip(hasher.finalize().iter()) {
            *slot = *byte;
        }
        to_lower_hex(&prefix)
    }
}

const _: () = assert!(
    CACHE_DIRECTORY_DIGEST_BYTES <= 32,
    "the cache directory digest cannot be wider than a SHA-256 digest"
);

/// Computes a hash of the workspace `Cargo.lock`, if present.
pub fn lockfile_fingerprint(workspace_root: &Utf8Path) -> Result<Option<String>, OrthohelpError> {
    let dir = Dir::open_ambient_dir(workspace_root, ambient_authority()).map_err(|err| {
        OrthohelpError::Io {
            path: workspace_root.to_path_buf(),
            source: err,
        }
    })?;
    let mut file = match dir.open("Cargo.lock") {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(OrthohelpError::Io {
                path: workspace_root.join("Cargo.lock"),
                source: err,
            });
        }
    };
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|err| OrthohelpError::Io {
            path: workspace_root.join("Cargo.lock"),
            source: err,
        })?;
    let mut hasher = Sha256::new();
    hasher.update(&buffer);
    Ok(Some(to_lower_hex(&hasher.finalize())))
}

/// Computes a fingerprint over the package inputs that influence the IR.
pub fn fingerprint_package(package_root: &Utf8Path) -> Result<String, OrthohelpError> {
    let dir = Dir::open_ambient_dir(package_root, ambient_authority()).map_err(|err| {
        OrthohelpError::Io {
            path: package_root.to_path_buf(),
            source: err,
        }
    })?;
    let mut hasher = Sha256::new();

    hash_file_if_present(
        &dir,
        Utf8Path::new("Cargo.toml"),
        Utf8Path::new("Cargo.toml"),
        &mut hasher,
    )?;
    hash_file_if_present(
        &dir,
        Utf8Path::new("build.rs"),
        Utf8Path::new("build.rs"),
        &mut hasher,
    )?;
    hash_directory_if_present(&dir, Utf8Path::new("src"), &mut hasher)?;
    hash_directory_if_present(&dir, Utf8Path::new("locales"), &mut hasher)?;

    Ok(to_lower_hex(&hasher.finalize()))
}

/// Add an existing file's relative name and contents to the package digest.
///
/// A missing input contributes nothing, while open or read failures are
/// reported with `hash_path` for diagnostics. The directory handle and hasher
/// remain borrowed; the opened file and its read buffer are owned locally.
fn hash_file_if_present(
    dir: &Dir,
    open_path: &Utf8Path,
    hash_path: &Utf8Path,
    hasher: &mut Sha256,
) -> Result<(), OrthohelpError> {
    let mut file = match dir.open(open_path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(OrthohelpError::Io {
                path: hash_path.to_path_buf(),
                source: err,
            });
        }
    };

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|err| OrthohelpError::Io {
            path: hash_path.to_path_buf(),
            source: err,
        })?;

    hasher.update(hash_path.as_str().as_bytes());
    hasher.update(&buffer);
    Ok(())
}

/// Hash a directory tree when its root exists, treating absence as no input.
///
/// The opened subdirectory is owned for the recursive walk and dropped on
/// return; filesystem errors propagate with the affected relative path.
fn hash_directory_if_present(
    dir: &Dir,
    path: &Utf8Path,
    hasher: &mut Sha256,
) -> Result<(), OrthohelpError> {
    if let Some(subdir) = try_open_dir(dir, path)? {
        hash_directory_recursive(&subdir, path, hasher)?;
    }
    Ok(())
}

/// Hash regular files under a directory in deterministic relative-path order.
///
/// Entries are sorted before traversal so filesystem enumeration order cannot
/// change the cache key. Directory entries recurse, regular files contribute
/// their relative path and bytes, and other entry kinds are skipped. Any read
/// or metadata failure aborts fingerprinting with the corresponding path.
fn hash_directory_recursive(
    dir: &Dir,
    base: &Utf8Path,
    hasher: &mut Sha256,
) -> Result<(), OrthohelpError> {
    let mut entries = Vec::new();
    for entry_result in dir.read_dir(".").map_err(|err| OrthohelpError::Io {
        path: base.to_path_buf(),
        source: err,
    })? {
        let entry = entry_result.map_err(|err| OrthohelpError::Io {
            path: base.to_path_buf(),
            source: err,
        })?;
        let entry_name = entry.file_name().map_err(|err| OrthohelpError::Io {
            path: base.to_path_buf(),
            source: err,
        })?;
        let file_name = Utf8PathBuf::from(entry_name);
        let file_type = entry.file_type().map_err(|err| OrthohelpError::Io {
            path: base.to_path_buf(),
            source: err,
        })?;
        entries.push((file_name, file_type));
    }

    entries.sort_by(|(left, _), (right, _)| left.cmp(right));

    for (name, file_type) in entries {
        let rel = base.join(&name);
        if file_type.is_dir() {
            let subdir = dir.open_dir(&name).map_err(|err| OrthohelpError::Io {
                path: rel.clone(),
                source: err,
            })?;
            hash_directory_recursive(&subdir, &rel, hasher)?;
        } else if file_type.is_file() {
            hash_file_if_present(dir, &name, &rel, hasher)?;
        }
    }

    Ok(())
}

/// Open a directory relative to `dir`, distinguishing absence from I/O failure.
///
/// The returned handle owns the opened directory capability. A missing path
/// becomes `None`; other failures retain the requested path in the error.
fn try_open_dir(dir: &Dir, path: &Utf8Path) -> Result<Option<Dir>, OrthohelpError> {
    match dir.open_dir(path) {
        Ok(subdir) => Ok(Some(subdir)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(OrthohelpError::Io {
            path: path.to_path_buf(),
            source: err,
        }),
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
