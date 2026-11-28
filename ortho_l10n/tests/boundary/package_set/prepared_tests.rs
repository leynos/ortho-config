//! What the preparation record vouches for, and what makes it stale.
//!
//! Each case builds a small stand-in repository and packaging target in a
//! temporary directory and calls the record functions with an injected
//! toolchain identity, so nothing spawns Cargo and nothing reads the process
//! environment.

use super::{check, invalidate, write};
use crate::boundary::package_set::{PACKAGED_CRATES, PACKAGED_VERSION};
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;

const TOOLCHAIN: &str = "cargo 1.0.0; rustc 1.0.0";

/// A stand-in repository with a prepared packaging target beside it.
struct Scene {
    _directory: tempfile::TempDir,
    repository: Utf8PathBuf,
    target: Utf8PathBuf,
}

impl Scene {
    fn new() -> Result<Self> {
        let directory = tempfile::tempdir().context("create a scene")?;
        let repository = Utf8PathBuf::from_path_buf(directory.path().join("repo"))
            .map_err(|path| anyhow::anyhow!("{} is not UTF-8", path.display()))?;
        let target = repository.join("target/localization-boundary/packaging-target");
        let scene = Self {
            _directory: directory,
            repository,
            target,
        };
        scene.populate()?;
        Ok(scene)
    }

    fn populate(&self) -> Result<()> {
        for member in [
            "ortho_l10n",
            "ortho_config",
            "ortho_config_macros",
            "test_helpers",
        ] {
            self.put(&format!("{member}/Cargo.toml"), "[package]\n")?;
            self.put(&format!("{member}/src/lib.rs"), "// source\n")?;
        }
        self.put("Cargo.toml", "[workspace]\n")?;
        self.put("Cargo.lock", "# lock\n")?;
        for (name, version) in PACKAGED_CRATES {
            self.put_in_target(&format!("package/{name}-{version}.crate"), "archive")?;
            self.put_in_target(&format!("package/{name}-{version}/Cargo.toml"), "manifest")?;
        }
        Ok(())
    }

    fn put(&self, relative: &str, contents: &str) -> Result<()> {
        write_under(&self.repository, relative, contents)
    }

    fn put_in_target(&self, relative: &str, contents: &str) -> Result<()> {
        write_under(&self.target, relative, contents)
    }

    fn prepare(&self) -> Result<()> {
        write(&self.repository, &self.target, TOOLCHAIN)
    }

    fn check(&self, toolchain: &str) -> Result<()> {
        check(&self.repository, &self.target, toolchain)
    }
}

/// Write `contents` to `relative` beneath `root`, creating parents.
fn write_under(root: &Utf8Path, relative: &str, contents: &str) -> Result<()> {
    Dir::create_ambient_dir_all(root, cap_std::ambient_authority())?;
    let directory = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    if let Some(parent) = Utf8Path::new(relative).parent() {
        directory.create_dir_all(parent)?;
    }
    directory
        .write(relative, contents)
        .with_context(|| format!("write {relative}"))
}

/// The error text of a refused check, or a failure if it was accepted.
fn refusal(result: Result<()>) -> Result<String> {
    match result {
        Ok(()) => anyhow::bail!("the record was accepted but should have been refused"),
        Err(error) => Ok(format!("{error:#}")),
    }
}

/// An unprepared suite is refused with the command that prepares it.
#[test]
fn a_missing_record_names_the_command_that_writes_it() -> Result<()> {
    let scene = Scene::new()?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("make boundary-prepare") && message.contains("no record"),
        "a missing record should be named and the fix given: {message}",
    );
    Ok(())
}

/// A record written for the current sources is accepted.
#[test]
fn a_record_for_unchanged_inputs_is_accepted() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.check(TOOLCHAIN)
}

/// Editing a packaged source invalidates the record.
#[test]
fn a_changed_source_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put("ortho_l10n/src/lib.rs", "// edited\n")?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("stale")
            && message.contains("make boundary-prepare")
            && message.contains("changed ortho_l10n/src/lib.rs"),
        "an edited source should read as stale and be named: {message}",
    );
    Ok(())
}

/// A new file in a packaged member invalidates the record: Cargo would package it.
#[test]
fn a_new_packaged_file_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put("ortho_config/locales/en/extra.ftl", "key = value\n")?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("added ortho_config/locales/en/extra.ftl"),
        "an added file should be named: {message}",
    );
    Ok(())
}

/// A stale record names at most a handful of paths and counts the rest.
#[test]
fn a_long_list_of_changes_is_summarized() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    for index in 0..20 {
        scene.put(&format!("ortho_config/extra/file-{index}.txt"), "new\n")?;
    }
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("and 12 more: 12 x added ortho_config/extra/*.txt"),
        "twenty added files should name eight and count twelve: {message}",
    );
    Ok(())
}

/// The lockfile decides what verification resolved.
#[test]
fn a_changed_lockfile_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put("Cargo.lock", "# another lock\n")?;
    refusal(scene.check(TOOLCHAIN)).map(|_message| ())
}

/// A different compiler did not verify these packages.
#[test]
fn a_different_toolchain_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    let message = refusal(scene.check("cargo 2.0.0; rustc 2.0.0"))?;
    ensure!(
        message.contains("toolchain differs"),
        "a different toolchain should be named: {message}",
    );
    Ok(())
}

/// Build output and hidden entries are not packaged, so they do not count.
#[test]
fn build_output_and_hidden_entries_do_not_change_the_fingerprint() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put("ortho_l10n/target/debug/artefact", "output\n")?;
    scene.put("ortho_l10n/.hidden/state", "state\n")?;
    scene.check(TOOLCHAIN)
}

/// Profile data a coverage run writes into a member directory is run output.
///
/// The first CI run of the preparation failed on exactly these files, which an
/// instrumented test binary leaves in its working directory after the record
/// was written.
#[test]
fn coverage_profile_data_does_not_change_the_fingerprint() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put("ortho_config/default_123_0_456.profraw", "raw\n")?;
    scene.put("ortho_config/merged.profdata", "merged\n")?;
    scene.check(TOOLCHAIN)
}

/// A replaced archive is not the one that passed verification.
#[test]
fn a_replaced_archive_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    let (name, version) = PACKAGED_CRATES
        .first()
        .copied()
        .context("the packaged set is empty")?;
    scene.put_in_target(&format!("package/{name}-{version}.crate"), "replaced")?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("archive"),
        "a replaced archive should be named: {message}",
    );
    Ok(())
}

/// A replaced unpacked manifest is not the one the verified archive held.
#[test]
fn a_replaced_unpacked_manifest_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put_in_target(
        &format!("package/ortho_l10n-{PACKAGED_VERSION}/Cargo.toml"),
        "replaced",
    )?;
    refusal(scene.check(TOOLCHAIN)).map(|_message| ())
}

/// Preparation discards the old record before it replaces anything.
#[test]
fn invalidating_removes_the_record() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    invalidate(&scene.target)?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("no record"),
        "an invalidated record should read as missing: {message}",
    );
    Ok(())
}

/// Invalidating before any packaging has run is not an error.
#[test]
fn invalidating_a_target_that_does_not_exist_is_a_no_op() -> Result<()> {
    let scene = Scene::new()?;
    invalidate(&scene.repository.join("no-such-target"))
}
