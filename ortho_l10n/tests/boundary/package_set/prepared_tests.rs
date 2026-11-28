//! What the preparation record vouches for, and what makes it stale.
//!
//! Each case builds a small stand-in repository and packaging target in a
//! temporary directory and calls the record functions with an injected
//! toolchain identity, so nothing spawns Cargo and nothing reads the process
//! environment.

use super::{Snapshot, check, invalidate, write};
use crate::boundary::package_set::PACKAGED_CRATES;
use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::fs_utf8::Dir;

const TOOLCHAIN: &str = "cargo 1.0.0; rustc 1.0.0";

/// The packaged members the stand-in repository holds.
const MEMBERS: [&str; 4] = [
    "ortho_l10n",
    "ortho_config",
    "ortho_config_macros",
    "test_helpers",
];

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
        for member in MEMBERS {
            self.put(
                &Utf8PathBuf::from(member).join("Cargo.toml"),
                b"[package]\n",
            )?;
            self.put(
                &Utf8PathBuf::from(member).join("src/lib.rs"),
                b"// source\n",
            )?;
        }
        self.put(Utf8Path::new("Cargo.toml"), b"[workspace]\n")?;
        self.put(Utf8Path::new("Cargo.lock"), b"# lock\n")?;
        for (name, version) in PACKAGED_CRATES {
            let package = Utf8PathBuf::from("package");
            self.put_in_target(&package.join(format!("{name}-{version}.crate")), b"archive")?;
            let root = package.join(format!("{name}-{version}"));
            self.put_in_target(&root.join("Cargo.toml"), b"manifest")?;
            self.put_in_target(&root.join("src/lib.rs"), b"// packaged source\n")?;
        }
        Ok(())
    }

    fn put(&self, relative: &Utf8Path, contents: &[u8]) -> Result<()> {
        write_under(&self.repository, relative, contents)
    }

    fn put_in_target(&self, relative: &Utf8Path, contents: &[u8]) -> Result<()> {
        write_under(&self.target, relative, contents)
    }

    fn snapshot(&self) -> Result<Snapshot> {
        Snapshot::capture(&self.repository, TOOLCHAIN)
    }

    fn prepare(&self) -> Result<()> {
        write(self.snapshot()?, &self.target)
    }

    fn check(&self, toolchain: &str) -> Result<()> {
        check(&self.repository, &self.target, toolchain)
    }
}

/// Write `contents` to `relative` beneath `root`, creating parents.
fn write_under(root: &Utf8Path, relative: &Utf8Path, contents: &[u8]) -> Result<()> {
    Dir::create_ambient_dir_all(root, cap_std::ambient_authority())?;
    let directory = Dir::open_ambient_dir(root, cap_std::ambient_authority())?;
    if let Some(parent) = relative.parent() {
        directory.create_dir_all(parent)?;
    }
    directory
        .write(relative, contents)
        .with_context(|| format!("write {relative}"))
}

/// The packaged root of the first crate, beneath the packaging target.
fn first_packaged_root() -> Result<Utf8PathBuf> {
    let (name, version) = PACKAGED_CRATES
        .first()
        .copied()
        .context("the packaged set is empty")?;
    Ok(Utf8PathBuf::from("package").join(format!("{name}-{version}")))
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
    scene.put(Utf8Path::new("ortho_l10n/src/lib.rs"), b"// edited\n")?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("stale")
            && message.contains("make boundary-prepare")
            && message.contains("changed ortho_l10n/src/lib.rs"),
        "an edited source should read as stale and be named: {message}",
    );
    Ok(())
}

/// A source edited while packaging ran is recorded as it was before.
///
/// The snapshot is taken before packaging and published after, so an edit in
/// between leaves archives that may hold it and a record that does not: the
/// first check must refuse, not vouch for an unverified tree.
#[test]
fn a_source_edited_while_packaging_ran_is_refused() -> Result<()> {
    let scene = Scene::new()?;
    let snapshot = scene.snapshot()?;
    scene.put(
        Utf8Path::new("ortho_l10n/src/lib.rs"),
        b"// edited during\n",
    )?;
    write(snapshot, &scene.target)?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("changed ortho_l10n/src/lib.rs"),
        "an edit made between capture and publication should be named: {message}",
    );
    Ok(())
}

/// A new file in a packaged member invalidates the record: Cargo would package it.
#[test]
fn a_new_packaged_file_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put(
        Utf8Path::new("ortho_config/locales/en/extra.ftl"),
        b"key = value\n",
    )?;
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
        let path = Utf8PathBuf::from(format!("ortho_config/extra/file-{index}.txt"));
        scene.put(&path, b"new\n")?;
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
    scene.put(Utf8Path::new("Cargo.lock"), b"# another lock\n")?;
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

/// Build output is not packaged, so it does not count.
#[test]
fn build_output_does_not_change_the_digests() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put(
        Utf8Path::new("ortho_l10n/target/debug/artefact"),
        b"output\n",
    )?;
    scene.put(Utf8Path::new("ortho_l10n/.git/state"), b"state\n")?;
    scene.check(TOOLCHAIN)
}

/// Cargo packages a tracked hidden file, and a build can embed one, so it counts.
#[test]
fn a_hidden_source_is_covered() -> Result<()> {
    let scene = Scene::new()?;
    scene.put(
        Utf8Path::new("ortho_l10n/src/.catalogue.ftl"),
        b"key = one\n",
    )?;
    scene.prepare()?;
    scene.put(
        Utf8Path::new("ortho_l10n/src/.catalogue.ftl"),
        b"key = two\n",
    )?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("changed ortho_l10n/src/.catalogue.ftl"),
        "an edited hidden source should be named: {message}",
    );
    Ok(())
}

/// Profile data a coverage run writes into a member directory is run output.
///
/// The first CI run of the preparation failed on exactly these files, which an
/// instrumented test binary leaves in its working directory after the record
/// was written.
#[test]
fn coverage_profile_data_does_not_change_the_digests() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    scene.put(
        Utf8Path::new("ortho_config/default_123_0_456.profraw"),
        b"raw\n",
    )?;
    scene.put(Utf8Path::new("ortho_config/merged.profdata"), b"merged\n")?;
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
    let archive = Utf8PathBuf::from("package").join(format!("{name}-{version}.crate"));
    scene.put_in_target(&archive, b"replaced")?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains(&format!("changed {name}-{version}.crate")),
        "a replaced archive should be named: {message}",
    );
    Ok(())
}

/// A replaced unpacked manifest is not the one the verified archive held.
#[test]
fn a_replaced_unpacked_manifest_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    let root = first_packaged_root()?;
    scene.put_in_target(&root.join("Cargo.toml"), b"replaced")?;
    refusal(scene.check(TOOLCHAIN)).map(|_message| ())
}

/// An unpacked source is what the consumer builds compile, so it is bound too.
#[test]
fn a_replaced_unpacked_source_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    let root = first_packaged_root()?;
    scene.put_in_target(
        &root.join("src/lib.rs"),
        b"// replaced after verification\n",
    )?;
    let message = refusal(scene.check(TOOLCHAIN))?;
    ensure!(
        message.contains("src/lib.rs") && message.contains("make boundary-prepare"),
        "an edited unpacked source should be named: {message}",
    );
    Ok(())
}

/// A file added to an unpacked root is compiled too, so it is bound as well.
#[test]
fn a_file_added_to_an_unpacked_root_makes_the_record_stale() -> Result<()> {
    let scene = Scene::new()?;
    scene.prepare()?;
    let root = first_packaged_root()?;
    scene.put_in_target(&root.join("src/extra.rs"), b"// added\n")?;
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
