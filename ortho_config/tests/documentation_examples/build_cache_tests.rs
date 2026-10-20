//! Cross-process cache-lock ownership and deadline regressions.

use super::{BuildCache, BuildInputs, BuildRequest};
use crate::process_runner::{self, Operation, Policy};
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[test]
fn cache_selection_is_independent_of_process_environment() {
    let cache = BuildCache::for_parent_target(Path::new("configured-parent"));
    assert_eq!(
        cache.root,
        Path::new("configured-parent/documentation-examples/v2")
    );
    assert_eq!(cache.target, Path::new("configured-parent"));
    assert_ne!(cache.root, cache.target);
}

#[test]
fn another_process_cannot_enter_build_and_snapshot_ownership() -> Result<()> {
    let state = tempfile::tempdir()?;
    let cache = BuildCache::for_parent_target(state.path());
    let owner = cache.acquire(Instant::now() + Duration::from_secs(5))?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--ignored", "--nocapture", "cache_lock_probe"])
        .env("ORTHO_DOC_LOCK_PROBE", state.path());
    let output = process_runner::run_with_policy(
        &mut command,
        Operation("exercise separate-process lock contention"),
        Policy::new(
            "execution",
            Duration::from_secs(5),
            Duration::from_secs(2),
            1024 * 1024,
        ),
    )?;
    ensure!(output.status.success(), "lock probe failed: {output:?}");
    ensure!(String::from_utf8_lossy(&output.stdout).contains("CACHE_LOCK_BLOCKED"));
    drop(owner);
    cache.acquire(Instant::now() + Duration::from_secs(1))?;
    Ok(())
}

#[test]
#[ignore = "executed in a separate cache-lock process"]
fn cache_lock_probe() -> Result<()> {
    let root = std::env::var_os("ORTHO_DOC_LOCK_PROBE").context("lock probe root")?;
    let cache = BuildCache::for_parent_target(Path::new(&root));
    let error = cache
        .acquire(Instant::now() + Duration::from_millis(50))
        .expect_err("another process owns the stable lock inode");
    ensure!(
        error
            .to_string()
            .contains("waiting for build-and-snapshot lock")
    );
    writeln!(std::io::stdout().lock(), "\nCACHE_LOCK_BLOCKED")?;
    Ok(())
}

#[test]
fn cache_lock_is_not_unlinked_between_owners() -> Result<()> {
    let state = tempfile::tempdir()?;
    let cache = BuildCache::for_parent_target(state.path());
    let lock = cache.acquire(Instant::now() + Duration::from_secs(1))?;
    drop(lock);
    let directory = Dir::open_ambient_dir(&cache.root, ambient_authority())?;
    ensure!(directory.exists("build.lock"));
    Ok(())
}

#[test]
fn interrupted_build_requires_owner_recovery_without_reusing_its_paths() -> Result<()> {
    let state = tempfile::tempdir()?;
    let cache = BuildCache::for_parent_target(state.path());
    let lock = cache.acquire(Instant::now() + Duration::from_secs(1))?;
    let directory = Dir::open_ambient_dir(&cache.root, ambient_authority())?;
    directory.write("build.in-progress", "interrupted ownership")?;
    drop(lock);
    let started = Instant::now();
    let error = cache
        .build_in(
            BuildInputs {
                package: Path::new("does-not-exist"),
                destination: state.path(),
                arguments: &["--bins"],
            },
            started,
            Duration::from_secs(1),
        )
        .expect_err("a stale owner marker must fail before starting Cargo");
    ensure!(error.to_string().contains("needs owner recovery"));
    Ok(())
}

#[test]
fn direct_build_preparation_error_releases_cache_ownership() -> Result<()> {
    let state = tempfile::tempdir()?;
    let directory = Dir::open_ambient_dir(state.path(), ambient_authority())?;
    directory.write("build.in-progress", "ordinary controlled failure")?;
    let error = super::finish_owned::<()>(&directory, Err(anyhow::anyhow!("preparation failed")))
        .expect_err("keep the ordinary preparation error");
    ensure!(error.root_cause().to_string() == "preparation failed");
    ensure!(!directory.exists("build.in-progress"));
    directory.write("build.in-progress", "next controlled owner")?;
    super::finish_owned(&directory, Ok(()))?;
    Ok(())
}

#[test]
fn opaque_nested_timeout_retains_marker_after_complete_outer_cleanup() -> Result<()> {
    let state = tempfile::tempdir()?;
    let cache = BuildCache::for_parent_target(state.path());
    let error = cache
        .run_owned_state(Instant::now() + Duration::from_secs(5), |_, _, _| {
            let mut command = Command::new(std::env::current_exe()?);
            command.args([
                "--ignored",
                "--quiet",
                "--nocapture",
                "opaque_nested_timeout_probe",
            ]);
            process_runner::run_with_policy(
                &mut command,
                Operation("opaque nested build probe"),
                Policy::new(
                    "build",
                    Duration::from_millis(100),
                    Duration::from_secs(2),
                    1024 * 1024,
                ),
            )
        })
        .expect_err("an opaque nested build timeout must require owner recovery");
    ensure!(process_runner::deadline_exceeded(&error));
    ensure!(!process_runner::cleanup_incomplete(&error));
    ensure!(
        error
            .root_cause()
            .to_string()
            .contains("subprocess timed out")
    );
    let directory = Dir::open_ambient_dir(&cache.root, ambient_authority())?;
    ensure!(directory.exists("build.in-progress"));
    let retry = cache
        .run_owned_state::<()>(Instant::now() + Duration::from_secs(1), |_, _, _| {
            anyhow::bail!("an unconfirmed nested compiler must not admit another owner")
        })
        .expect_err("the retained marker must reject the next owner");
    ensure!(retry.to_string().contains("needs owner recovery"));
    Ok(())
}

#[test]
fn interrupted_build_retains_marker_after_complete_owned_tree_cleanup() -> Result<()> {
    let state = tempfile::tempdir()?;
    let cache = BuildCache::for_parent_target(state.path());
    let error = cache
        .build_with(
            BuildRequest {
                package: state.path(),
                destination: state.path(),
                started: Instant::now(),
                timeout: Duration::from_millis(100),
            },
            |_, _| {
                let mut command = Command::new(std::env::current_exe()?);
                command.args(["--ignored", "--quiet", "opaque_nested_timeout_probe"]);
                Ok(command)
            },
        )
        .expect_err("an interrupted Cargo peer cannot prove external writers stopped");
    ensure!(process_runner::deadline_exceeded(&error));
    ensure!(!process_runner::cleanup_incomplete(&error));
    ensure!(
        error
            .to_string()
            .contains("external compiler writer lifetime")
    );
    let directory = Dir::open_ambient_dir(&cache.root, ambient_authority())?;
    ensure!(directory.exists("build.in-progress"));
    let next = cache
        .run_owned_state(Instant::now() + Duration::from_secs(1), |_, _, _| Ok(()))
        .expect_err("unconfirmed external writers must reject the next owner");
    ensure!(next.to_string().contains("needs owner recovery"));
    Ok(())
}

#[test]
#[ignore = "executed as a controlled opaque nested-build timeout"]
fn opaque_nested_timeout_probe() {
    std::thread::sleep(Duration::from_secs(2));
}

#[rstest::rstest]
#[case::wait("nested CLI wait failed")]
#[case::read("nested CLI pipe read failed")]
#[case::exit_status("nested CLI exited unsuccessfully")]
fn opaque_callback_failure_preserves_primary_error_and_ownership(
    #[case] failure: &str,
) -> Result<()> {
    let state = tempfile::tempdir()?;
    let cache = BuildCache::for_parent_target(state.path());
    let error = cache
        .run_owned_state::<()>(Instant::now() + Duration::from_secs(1), |_, _, _| {
            Err(anyhow::anyhow!(failure.to_owned()))
        })
        .expect_err("opaque errors cannot confirm nested compiler completion");
    ensure!(error.root_cause().to_string() == failure);
    ensure!(
        error
            .to_string()
            .contains("cache recovery requires confirmation")
    );
    let directory = Dir::open_ambient_dir(&cache.root, ambient_authority())?;
    ensure!(directory.exists("build.in-progress"));
    let retry = cache
        .run_owned_state(Instant::now() + Duration::from_secs(1), |_, _, _| Ok(()))
        .expect_err("the retained marker must reject the next owner");
    ensure!(retry.to_string().contains("needs owner recovery"));
    Ok(())
}

/// Check the invoking Cargo without starting another cold Cargo workspace.
///
/// The running test is already a separate process from Cargo. Snapshot ownership
/// excludes our documentation builds; another Cargo owner can still make the
/// observation inconclusive, so contention must not be blamed on the parent.
#[test]
fn invoking_cargo_releases_its_profile_lock_before_running_tests() -> Result<()> {
    let deadline = super::scenario::deadline();
    let executable = std::env::current_exe()?;
    let dependencies = executable.parent().context("test executable directory")?;
    ensure!(
        dependencies.file_name().is_some_and(|name| name == "deps"),
        "cannot identify Cargo profile from test executable: {}",
        executable.display()
    );
    let profile = dependencies.parent().context("Cargo profile directory")?;
    let target = profile.parent().context("Cargo target directory")?;
    let cache = BuildCache::for_parent_target(target);
    let _snapshot_owner = cache.acquire(deadline)?;
    let directory = Dir::open_ambient_dir(profile, ambient_authority())?;
    let lock = directory
        .open_with(
            ".cargo-lock",
            cap_std::fs::OpenOptions::new().read(true).write(true),
        )?
        .into_std();
    fs4::FileExt::try_lock(&lock).map_err(|error| {
        anyhow::anyhow!(
            concat!(
                "unable to verify Cargo profile-lock release at {}: {}; ",
                "another Cargo process may own this lock"
            ),
            profile.display(),
            error
        )
    })?;
    Ok(())
}
