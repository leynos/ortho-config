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
    CACHEDIR_TAG, FixtureLocation, TARGET_DIRECTORY_NAME, location_from_metadata, private_target,
};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// The fixture package, as Cargo reports it in `packages[].id`.
const FIXTURE: &str = "path+file:///fixture#orthohelp_fixture@0.1.0";

/// How long the regression's lease may take; nothing else holds the target.
const LEASE_WAIT: Duration = Duration::from_secs(30);

/// The lease file the target must contain once it has been prepared.
const LEASE_FILE_NAME: &str = ".identifier-artefact.lease";

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
