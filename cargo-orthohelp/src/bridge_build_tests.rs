//! Behavioural coverage for private bridge build ownership.

#[path = "bridge_build_test_support.rs"]
mod support;

use anyhow::{Result, ensure};
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, OpenOptions};
use rstest::rstest;
use std::cell::Cell;
use std::io::Write;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

use super::MARKER_FILE;

#[test]
fn reported_custom_target_executable_is_snapshotted() -> Result<()> {
    let temporary = tempdir()?;
    let target = support::utf8_path(temporary.path().join("target"))?;
    let snapshot = support::utf8_path(temporary.path().join("private-run"))?;

    let executable = support::build_probe("artifact", &target, &snapshot)?;
    let content = support::read_text(&executable)?;

    ensure!(executable == snapshot.join(support::binary_file_name()));
    ensure!(content == "bridge fixture");
    support::ensure_executable(&executable)?;
    ensure!(!support::path_exists(&target.join(MARKER_FILE))?);
    ensure!(support::path_exists(
        &target
            .join("wasm32-unknown-unknown/release")
            .join(support::binary_file_name())
    )?);
    Ok(())
}

#[rstest]
#[case("malformed", "invalid Cargo JSON record")]
#[case("oversized", "exceeded 128 KiB")]
fn invalid_records_fail_and_clear_marker(#[case] mode: &str, #[case] expected: &str) -> Result<()> {
    let temporary = tempdir()?;
    let target = support::utf8_path(temporary.path().join("target"))?;
    let snapshot = support::utf8_path(temporary.path().join("private-run"))?;
    support::create_directory_all(&snapshot)?;
    let result = support::build_probe(mode, &target, &snapshot);

    let error = result
        .err()
        .ok_or_else(|| anyhow::anyhow!("invalid Cargo stream succeeded"))?;
    ensure!(error.to_string().contains(expected));
    ensure!(!support::path_exists(&target.join(MARKER_FILE))?);
    Ok(())
}

#[test]
fn stale_marker_blocks_build_without_removal() -> Result<()> {
    let temporary = tempdir()?;
    let target = support::utf8_path(temporary.path().join("target"))?;
    let snapshot = support::utf8_path(temporary.path().join("private-run"))?;
    support::create_directory_all(&target)?;
    support::create_directory_all(&snapshot)?;
    let target_dir = Dir::open_ambient_dir(&target, ambient_authority())?;
    target_dir
        .open_with(MARKER_FILE, OpenOptions::new().write(true).create_new(true))?
        .write_all(b"interrupted owner")?;
    let runner_called = Cell::new(false);
    let result = super::build_and_snapshot_with(&target, &snapshot, || {
        runner_called.set(true);
        Err(super::CargoFailure::confirmed(
            crate::error::OrthohelpError::Message("unexpected Cargo runner call".to_owned()),
        ))
    });

    ensure!(result.is_err());
    ensure!(!runner_called.get());
    ensure!(support::path_exists(&target.join(MARKER_FILE))?);
    Ok(())
}

#[test]
fn separate_processes_snapshot_same_named_artifacts_privately() -> Result<()> {
    let temporary = tempdir()?;
    let root = support::utf8_path(temporary.path().to_path_buf())?;
    let target = root.join("target");
    let coordination = root.join("coordination");
    let snapshot_a = root.join("snapshot-a");
    let snapshot_b = root.join("snapshot-b");
    for path in [&target, &coordination, &snapshot_a, &snapshot_b] {
        support::create_directory_all(path)?;
    }
    let mut first = support::owner_process("A", &target, &coordination, &snapshot_a)?;
    let first_child = support::ProbeProcess::spawn(&mut first)?;
    let release_first_owner = support::release_owner_on_drop(&coordination);
    support::wait_for_file(&coordination.join("started-A"))?;

    let mut second = support::owner_process("B", &target, &coordination, &snapshot_b)?;
    let second_child = support::ProbeProcess::spawn(&mut second)?;
    support::wait_for_file(&coordination.join("attempted-B"))?;
    thread::sleep(Duration::from_millis(250));
    let started_while_locked = support::path_exists(&coordination.join("started-B"))?;
    support::write_bytes(&coordination.join("release-A"), b"release")?;
    drop(release_first_owner);

    let first_result = first_child.wait();
    let second_result = second_child.wait();
    first_result?;
    second_result?;
    ensure!(
        !started_while_locked,
        "second build started while the lock was held"
    );
    ensure!(support::read_text(&snapshot_a.join(support::binary_file_name()))? == "A");
    ensure!(support::read_text(&snapshot_b.join(support::binary_file_name()))? == "B");
    Ok(())
}

#[test]
#[ignore = "invoked as a separate lock owner by bridge-build tests"]
fn cross_process_owner_probe() -> Result<()> {
    support::run_owner_process()
}
