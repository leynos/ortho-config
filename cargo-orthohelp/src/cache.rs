//! Cache key and fingerprint helpers for `cargo-orthohelp`.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use sha2::{Digest, Sha256};
use std::io::Read;

use crate::error::OrthohelpError;

/// Cache key inputs for the bridge IR.
#[derive(Debug, Clone)]
pub struct CacheKey {
    fingerprint: String,
    root_type: String,
    tool_version: String,
    ir_version: String,
}

impl CacheKey {
    /// Creates a new cache key input set.
    pub const fn new(
        fingerprint: String,
        root_type: String,
        tool_version: String,
        ir_version: String,
    ) -> Self {
        Self {
            fingerprint,
            root_type,
            tool_version,
            ir_version,
        }
    }

    /// Hashes the cache inputs into a stable identifier.
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.fingerprint.as_bytes());
        hasher.update(self.root_type.as_bytes());
        hasher.update(self.tool_version.as_bytes());
        hasher.update(self.ir_version.as_bytes());
        format!("{:x}", hasher.finalize())
    }
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

    Ok(format!("{:x}", hasher.finalize()))
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
    let subdir = match dir.open_dir(path) {
        Ok(subdir) => subdir,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(OrthohelpError::Io {
                path: path.to_path_buf(),
                source: err,
            });
        }
    };

    hash_directory_recursive(&subdir, path, hasher)
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

#[cfg(test)]
mod tests {
    //! Tests for cache fingerprinting.

    use super::*;
    use cap_std::fs_utf8::OpenOptions;
    use rstest::rstest;
    use std::io::Write;

    #[rstest]
    fn fingerprint_changes_on_file_update() {
        let tempdir = tempfile::tempdir().expect("create temp dir");
        let root = Utf8PathBuf::from_path_buf(tempdir.path().to_path_buf())
            .expect("tempdir path is UTF-8");
        let dir = Dir::open_ambient_dir(&root, ambient_authority()).expect("open temp dir");
        dir.create_dir_all("src").expect("create src directory");

        write_file(&dir, "Cargo.toml", "[package]\\nname = \"demo\"\\n");
        write_file(&dir, "src/lib.rs", "pub fn demo() -> u32 { 1 }\\n");

        let first = fingerprint_package(&root).expect("fingerprint");
        write_file(&dir, "src/lib.rs", "pub fn demo() -> u32 { 2 }\\n");
        let second = fingerprint_package(&root).expect("fingerprint after update");

        assert_ne!(first, second, "fingerprint should change when files change");
    }

    fn write_file(dir: &Dir, path: &str, contents: &str) {
        let mut file = dir
            .open_with(
                path,
                OpenOptions::new().write(true).create(true).truncate(true),
            )
            .expect("open file");
        file.write_all(contents.as_bytes()).expect("write file");
    }
}
