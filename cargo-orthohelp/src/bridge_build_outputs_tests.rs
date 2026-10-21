//! Cleanup regressions for generated bridge build outputs.
//!
//! The outcome is evidence the caller reports, so each case pins one
//! distinguishable state rather than only the happy path.

use super::outputs::{CleanupOutcome, remove_generated_outputs};
use super::protocol::binary_file_name;
use anyhow::{Context, Result, ensure};
use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use std::io::Write;
use tempfile::TempDir;

const TARGET_NAME: &str = "orthohelp_bridge_fixture";

/// Removing every reported output leaves no bridge artefact behind.
#[test]
fn reported_bridge_outputs_are_removed_from_the_target() -> Result<()> {
    let fixture = CleanupFixture::new()?;
    let executable = fixture.write_output(&binary_file_name(TARGET_NAME))?;
    let dep_info = fixture.write_output(&format!("{TARGET_NAME}.d"))?;

    let outcome = fixture.remove(&[executable.clone(), dep_info.clone()]);

    ensure!(outcome == CleanupOutcome::Removed);
    ensure!(outcome.is_complete());
    ensure!(!fixture.exists(&executable)?);
    ensure!(!fixture.exists(&dep_info)?);
    Ok(())
}

/// A target directory that cannot be opened reports that, not success.
///
/// This is the case that previously returned without any evidence: the caller
/// could not tell a cleaned target from one it never managed to inspect.
#[test]
fn an_unopenable_target_directory_is_reported_rather_than_assumed_clean() -> Result<()> {
    let fixture = CleanupFixture::new()?;
    let missing_target = fixture.root.join("absent-target");
    let reported = missing_target.join(binary_file_name(TARGET_NAME));

    let outcome = remove_generated_outputs(&missing_target, TARGET_NAME, &[reported]);

    ensure!(outcome == CleanupOutcome::TargetUnreadable);
    ensure!(!outcome.is_complete());
    Ok(())
}

/// An output that survives removal is reported instead of logged and dropped.
#[cfg(unix)]
#[test]
fn an_output_that_cannot_be_removed_is_reported() -> Result<()> {
    let fixture = CleanupFixture::new()?;
    let protected = fixture.write_output(&format!("{TARGET_NAME}.d"))?;
    fixture.deny_removal()?;

    let outcome = fixture.remove(std::slice::from_ref(&protected));

    ensure!(outcome == CleanupOutcome::OutputsRemaining);
    ensure!(!outcome.is_complete());
    ensure!(fixture.exists(&protected)?);
    Ok(())
}

/// A reported path owned by another binary is neither removed nor a failure.
#[test]
fn outputs_of_another_binary_are_left_untouched() -> Result<()> {
    let fixture = CleanupFixture::new()?;
    let foreign = fixture.write_output(&format!("{TARGET_NAME}_other.d"))?;

    let outcome = fixture.remove(std::slice::from_ref(&foreign));

    ensure!(outcome.is_complete());
    ensure!(fixture.exists(&foreign)?);
    Ok(())
}

struct CleanupFixture {
    _temporary_root: TempDir,
    root: Utf8PathBuf,
    target_path: Utf8PathBuf,
    target: Dir,
}

impl CleanupFixture {
    fn new() -> Result<Self> {
        let temporary_root = TempDir::new().context("create cleanup fixture root")?;
        let root = Utf8PathBuf::from_path_buf(temporary_root.path().to_path_buf())
            .map_err(|_| anyhow::anyhow!("temporary test path was not UTF-8"))?;
        let target_path = root.join("target");
        Dir::create_ambient_dir_all(&target_path, ambient_authority())?;
        let target = Dir::open_ambient_dir(&target_path, ambient_authority())?;
        Ok(Self {
            _temporary_root: temporary_root,
            root,
            target_path,
            target,
        })
    }

    fn remove(&self, outputs: &[Utf8PathBuf]) -> CleanupOutcome {
        remove_generated_outputs(&self.target_path, TARGET_NAME, outputs)
    }

    fn write_output(&self, name: &str) -> Result<Utf8PathBuf> {
        let mut file = self
            .target
            .open_with(name, OpenOptions::new().write(true).create_new(true))?;
        file.write_all(b"generated bridge output")?;
        Ok(self.target_path.join(name))
    }

    fn exists(&self, path: &Utf8PathBuf) -> Result<bool> {
        let name = path
            .file_name()
            .context("fixture output should have a file name")?;
        Ok(self.target.symlink_metadata(name).is_ok())
    }

    /// Make the containing directory read-only so removal must fail.
    ///
    /// `remove_file` needs write permission on the directory, not on the file,
    /// so changing the file's own mode would leave the test vacuous.
    #[cfg(unix)]
    fn deny_removal(&self) -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let mut permissions = std::fs::metadata(&self.target_path)?.permissions();
        permissions.set_mode(0o555);
        std::fs::set_permissions(&self.target_path, permissions)?;
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for CleanupFixture {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt as _;

        // Restore write access so TempDir can remove its tree.
        if let Ok(metadata) = std::fs::metadata(&self.target_path) {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o755);
            let _restored = std::fs::set_permissions(&self.target_path, permissions);
        }
    }
}
