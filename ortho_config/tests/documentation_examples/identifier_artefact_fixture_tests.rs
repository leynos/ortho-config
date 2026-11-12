//! Discovery must read only; the lease owns every write to the target.
//!
//! Two test processes reach the identifier-artefact fixture at once: nextest
//! starts the target's tests in parallel, and a second `cargo test` or
//! `make test` can run the same binary from another runner. Discovery is the
//! part that runs before the lease is held, so it must not create the target
//! directory or tag it — otherwise the second process writes beside the first
//! leaseholder before either has been excluded. These regressions hand
//! discovery a target path that does not exist and assert that it is still
//! absent afterwards, then take the lease and observe it created and marked
//! the directory itself.

use super::{
    CACHEDIR_TAG, FixtureLocation, TARGET_DIRECTORY_NAME, discovery_command, locate_fixture_with,
    location_from_metadata, private_target,
};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use serde_json::json;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// The fixture package, as Cargo reports it in `packages[].id`.
const FIXTURE: &str = "path+file:///fixture#orthohelp_fixture@0.1.0";

/// How long the regression's lease may take; nothing else holds the target.
const LEASE_WAIT: Duration = Duration::from_secs(30);

/// The lease file the target must contain once it has been prepared.
const LEASE_FILE_NAME: &str = ".identifier-artefact.lease";

/// How long full discovery may take.
///
/// Discovery runs `cargo metadata`, which queues on Cargo's package-cache lock
/// behind other work on a shared host, so the bound is generous; it exists to
/// fail with a diagnostic rather than to accommodate a busy machine.
const DISCOVERY_WAIT: Duration = Duration::from_secs(120);

/// Parsing metadata names the private target without creating anything.
///
/// The target path is derived from a workspace target that does not exist, so
/// a discovery helper that created or tagged its result would be visible here.
/// The same path is created by the lease regression below, which keeps this
/// assertion from passing merely because nothing ever creates the directory.
#[test]
fn parsing_metadata_for_a_fresh_target_creates_nothing() -> Result<()> {
    let scratch = TempDir::new().context("create the workspace target parent")?;
    let workspace_target = scratch.path().join("workspace-target");
    let metadata = json!({
        "target_directory": workspace_target.display().to_string(),
        "packages": [{"name": "orthohelp_fixture", "id": FIXTURE}],
    });

    let location = location_from_metadata(PathBuf::from("/repository"), &metadata)?;

    ensure!(
        location.target_directory == private_target(&workspace_target),
        "discovery should place the private target under the workspace target, not {}",
        location.target_directory.display(),
    );
    ensure!(
        location.package_id == FIXTURE,
        "discovery should keep the fixture package id, not {}",
        location.package_id,
    );
    let parent = Dir::open_ambient_dir(scratch.path(), ambient_authority())
        .context("open the workspace target parent")?;
    ensure!(
        !parent.exists("workspace-target"),
        "discovery created {} before any lease was held",
        workspace_target.display(),
    );
    Ok(())
}

/// Full discovery against a fresh target creates neither the target nor a file.
///
/// The regression above stops at the metadata reading, so a write made while
/// preparing or running the discovery command would not be visible to it. This
/// one runs the whole path — the command, its closed environment, the status
/// check, the JSON parse and the package lookup — with `CARGO_TARGET_DIR`
/// pointing under a fresh temporary directory. The private target must be
/// derived from that path without being created; Cargo's `metadata` does not
/// create a target directory, so anything present afterwards was discovery's.
#[test]
fn full_discovery_for_a_fresh_target_creates_nothing() -> Result<()> {
    let scratch = TempDir::new().context("create the discovery parent")?;
    let workspace_target = scratch.path().join("workspace-target");
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("find fixture workspace")?
        .to_path_buf();
    let mut command = discovery_command(&repository);
    // Set after the allow-list is applied, so the injected target wins over
    // whatever target directory this test process inherited.
    command.env("CARGO_TARGET_DIR", &workspace_target);

    let location = locate_fixture_with(Instant::now() + DISCOVERY_WAIT, repository, &mut command)?;

    ensure!(
        location.target_directory == private_target(&workspace_target),
        "discovery should place the private target under the injected target, not {}",
        location.target_directory.display(),
    );
    ensure!(
        location.package_id.contains("orthohelp_fixture"),
        "discovery should keep the fixture package id, not {}",
        location.package_id,
    );
    let parent = Dir::open_ambient_dir(scratch.path(), ambient_authority())
        .context("open the injected target parent")?;
    ensure!(
        !parent.exists("workspace-target"),
        "full discovery created {} before any lease was held",
        workspace_target.display(),
    );
    Ok(())
}

/// Taking the lease creates the private target and marks it as cache output.
#[test]
fn the_lease_creates_and_marks_the_private_target() -> Result<()> {
    let scratch = TempDir::new().context("create the fixture parent")?;
    let location = FixtureLocation {
        repository: PathBuf::from("/repository"),
        target_directory: scratch.path().join(TARGET_DIRECTORY_NAME),
        package_id: FIXTURE.to_owned(),
    };
    let target = location.target_directory.clone();

    let cargo = location.lease(Instant::now() + LEASE_WAIT, None)?;
    drop(cargo);

    let directory =
        Dir::open_ambient_dir(&target, ambient_authority()).context("open the leased target")?;
    let tag = directory
        .read_to_string("CACHEDIR.TAG")
        .context("read the cache-directory tag")?;
    ensure!(
        tag == CACHEDIR_TAG,
        "the target should carry Cargo's cache tag:\n{tag}",
    );
    ensure!(
        directory.exists(LEASE_FILE_NAME),
        "the target should hold the lease file it was created with",
    );
    Ok(())
}

/// A lookup failure other than absence is reported, not read as "no artefact".
///
/// `Dir::exists` folds every error into `false`, so a fixture whose output
/// directory is unreadable would look like a build that chose not to emit. The
/// regression puts a regular file where the artefact's parent directory
/// belongs, which fails the lookup with a kind other than `NotFound` on every
/// supported platform.
#[test]
fn artefact_path_reports_non_not_found_lookup_errors() -> Result<()> {
    let output = TempDir::new().context("create a temporary output directory")?;
    Dir::open_ambient_dir(output.path(), ambient_authority())
        .context("open the temporary output directory")?
        .write("ortho-config", "not a directory")
        .context("write a file at the artefact directory path")?;

    let error = super::artefact_path(output.path())
        .err()
        .context("a non-directory artefact parent must not read as absence")?;
    let io_error = error
        .downcast_ref::<io::Error>()
        .context("the cause is the filesystem error")?;
    ensure!(
        io_error.kind() != io::ErrorKind::NotFound,
        "the invalid parent must report its filesystem error, got {:?}",
        io_error.kind()
    );
    ensure!(
        error
            .to_string()
            .contains(&output.path().display().to_string()),
        "the error names the output directory: {error}"
    );
    Ok(())
}
