//! Cache key and fingerprint helpers for `cargo-orthohelp`.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use sha2::{Digest, Sha256};
use std::io::Read;

use crate::error::OrthohelpError;
use crate::hex::to_lower_hex;

/// Cache key inputs for the bridge IR.
#[derive(Debug, Clone)]
pub struct CacheKey {
    pub(crate) fingerprint: String,
    pub(crate) root_type: String,
    pub(crate) tool_version: String,
    pub(crate) ir_version: String,
    pub(crate) lockfile_hash: Option<String>,
}

impl CacheKey {
    /// Hashes the cache inputs into a stable identifier.
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
        to_lower_hex(&hasher.finalize())
    }
}

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
mod tests {
    //! Tests for cache fingerprinting.

    use super::*;
    use cap_std::fs_utf8::OpenOptions;
    use rstest::{fixture, rstest};
    use std::io::Write;

    /// Error type for cache test helpers; kept generic since these helpers
    /// only surface diagnostics for test failures, not production callers.
    type CacheTestError = Box<dyn std::error::Error>;

    /// Well-known SHA-256 digest of `b"abc"`. Pinning a canonical vector keeps
    /// the digest rendering honest: a self-consistent but wrongly ordered or
    /// zero-truncated encoder would still satisfy change-detection assertions.
    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[rstest]
    fn fingerprint_changes_on_file_update(
        temp_root: Result<(tempfile::TempDir, Utf8PathBuf, Dir), CacheTestError>,
    ) {
        let (_tempdir, root, dir) = temp_root.expect("temp root fixture should build");
        dir.create_dir_all("src").expect("create src directory");

        write_file(&dir, "Cargo.toml", "[package]\nname = \"demo\"\n").expect("write Cargo.toml");
        write_file(&dir, "src/lib.rs", "pub fn demo() -> u32 { 1 }\n").expect("write src/lib.rs");

        let first = fingerprint_package(&root).expect("fingerprint initial package");
        write_file(&dir, "src/lib.rs", "pub fn demo() -> u32 { 2 }\n").expect("rewrite src/lib.rs");
        let second = fingerprint_package(&root).expect("fingerprint updated package");

        assert_ne!(first, second, "fingerprint should change when files change");
        assert!(
            is_sha256_hex(&first),
            "package fingerprint should render as lowercase hex: {first}"
        );
    }

    #[rstest]
    fn lockfile_fingerprint_matches_known_digest_vector(
        temp_root: Result<(tempfile::TempDir, Utf8PathBuf, Dir), CacheTestError>,
    ) {
        let (_tempdir, root, dir) = temp_root.expect("temp root fixture should build");
        write_file(&dir, "Cargo.lock", "abc").expect("write Cargo.lock");

        let fingerprint = lockfile_fingerprint(&root).expect("fingerprint lockfile");

        assert_eq!(fingerprint.as_deref(), Some(ABC_SHA256));
    }

    #[rstest]
    fn lockfile_fingerprint_is_absent_without_a_lockfile(
        temp_root: Result<(tempfile::TempDir, Utf8PathBuf, Dir), CacheTestError>,
    ) {
        let (_tempdir, root, _dir) = temp_root.expect("temp root fixture should build");

        let fingerprint = lockfile_fingerprint(&root).expect("fingerprint without lockfile");

        assert_eq!(fingerprint, None, "absent lockfile should not fingerprint");
    }

    #[rstest]
    fn cache_key_hash_renders_lowercase_hex() {
        let hash = cache_key().hash();

        assert!(
            is_sha256_hex(&hash),
            "cache key hash should be 64 lowercase hex digits: {hash}"
        );
    }

    #[rstest]
    #[case::fingerprint(CacheKey { fingerprint: "other".to_owned(), ..cache_key() })]
    #[case::root_type(CacheKey { root_type: "workspace".to_owned(), ..cache_key() })]
    #[case::tool_version(CacheKey { tool_version: "0.9.0".to_owned(), ..cache_key() })]
    #[case::ir_version(CacheKey { ir_version: "2".to_owned(), ..cache_key() })]
    #[case::lockfile_hash(CacheKey { lockfile_hash: None, ..cache_key() })]
    fn cache_key_hash_tracks_every_input(#[case] varied: CacheKey) {
        assert_ne!(
            cache_key().hash(),
            varied.hash(),
            "hash should change when any input changes"
        );
    }

    /// Baseline cache key whose fields each differ, so a hash that ignores or
    /// transposes one field is detectable.
    fn cache_key() -> CacheKey {
        CacheKey {
            fingerprint: "fingerprint".to_owned(),
            root_type: "package".to_owned(),
            tool_version: "0.8.0".to_owned(),
            ir_version: "1".to_owned(),
            lockfile_hash: Some("lockfile".to_owned()),
        }
    }

    /// Reports whether `value` is a 64-digit lowercase hexadecimal string, the
    /// shape every SHA-256 digest in this module must render as.
    fn is_sha256_hex(value: &str) -> bool {
        value.len() == 64
            && value
                .bytes()
                .all(|digit| digit.is_ascii_digit() || (b'a'..=b'f').contains(&digit))
    }

    #[fixture]
    fn temp_root() -> Result<(tempfile::TempDir, Utf8PathBuf, Dir), CacheTestError> {
        let tempdir = tempfile::tempdir()?;
        let root = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
            .map_err(|path| format!("tempdir path is not UTF-8: {}", path.display()))?;
        let dir = Dir::open_ambient_dir(&root, ambient_authority())?;
        Ok((tempdir, root, dir))
    }

    fn write_file(dir: &Dir, path: &str, contents: &str) -> Result<(), CacheTestError> {
        let mut file = dir.open_with(
            path,
            OpenOptions::new().write(true).create(true).truncate(true),
        )?;
        file.write_all(contents.as_bytes())?;
        Ok(())
    }
}
