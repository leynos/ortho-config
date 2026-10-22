//! Regressions for private bridge snapshots and their failure cleanup.

use super::super::{GeneratedTarget, protocol::binary_file_name};
use super::snapshot_executable;
use anyhow::{Result, ensure};
use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::io::{Read, Write};
use tempfile::TempDir;

const TARGET_NAME: &str = "orthohelp_bridge_fixture";

#[test]
fn an_existing_snapshot_is_preserved_on_output_collision() -> Result<()> {
    let fixture = SnapshotFixture::new()?;
    let executable_name = binary_file_name(TARGET_NAME);
    write_file(&fixture.target, &executable_name, b"built bridge")?;
    write_file(&fixture.snapshot, &executable_name, b"existing snapshot")?;
    let target = GeneratedTarget {
        directory: &fixture.target_path,
        binary_name: TARGET_NAME,
    };

    let result = snapshot_executable(
        &fixture.target_path.join(&executable_name),
        &target,
        &fixture.snapshot_path,
        &fixture.snapshot,
    );

    ensure!(result.is_err());
    let mut existing = fixture.snapshot.open(&executable_name)?;
    let mut contents = String::new();
    existing.read_to_string(&mut contents)?;
    ensure!(contents == "existing snapshot");
    Ok(())
}

#[cfg(unix)]
#[test]
fn incomplete_snapshot_is_removed_when_copying_a_directory_fails() -> Result<()> {
    let fixture = SnapshotFixture::new()?;
    let executable_name = binary_file_name(TARGET_NAME);
    fixture.target.create_dir(&executable_name)?;
    let target = GeneratedTarget {
        directory: &fixture.target_path,
        binary_name: TARGET_NAME,
    };

    let result = snapshot_executable(
        &fixture.target_path.join(&executable_name),
        &target,
        &fixture.snapshot_path,
        &fixture.snapshot,
    );

    ensure!(result.is_err());
    ensure!(matches!(
        fixture.snapshot.symlink_metadata(&executable_name),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    ));
    Ok(())
}

struct SnapshotFixture {
    _temporary_root: TempDir,
    target_path: Utf8PathBuf,
    snapshot_path: Utf8PathBuf,
    target: Dir,
    snapshot: Dir,
}

impl SnapshotFixture {
    fn new() -> Result<Self> {
        let temporary_root = TempDir::new()?;
        let root_path = Utf8PathBuf::from_path_buf(temporary_root.path().to_path_buf())
            .map_err(|_| anyhow::anyhow!("temporary test path was not UTF-8"))?;
        let target_path = root_path.join("target");
        let snapshot_path = root_path.join("snapshot");
        Dir::create_ambient_dir_all(&target_path, ambient_authority())?;
        Dir::create_ambient_dir_all(&snapshot_path, ambient_authority())?;
        let target = Dir::open_ambient_dir(&target_path, ambient_authority())?;
        let snapshot = Dir::open_ambient_dir(&snapshot_path, ambient_authority())?;
        Ok(Self {
            _temporary_root: temporary_root,
            target_path,
            snapshot_path,
            target,
            snapshot,
        })
    }
}

fn write_file(directory: &Dir, name: &str, contents: &[u8]) -> Result<()> {
    let mut file = directory.open_with(name, OpenOptions::new().write(true).create_new(true))?;
    file.write_all(contents)?;
    Ok(())
}
