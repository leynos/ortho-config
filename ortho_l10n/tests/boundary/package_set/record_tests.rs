//! The record's read side is a query, and stays one.
//!
//! [`read_unlocked`] is the one record operation that runs with no lease to
//! lean on: it is the arrival check, called before any lock exists, which is
//! why it takes the directory directly rather than a [`PackagingLease`]. That
//! makes it the operation with no proof attached, so the one property worth
//! pinning about it is the one nothing else enforces - that asking a question
//! does not change the answer.
//!
//! The write side's properties are pinned where they are cheaper to express:
//! [`super::lease_tests`] drives the recorded scope through real leases, and
//! [`super::roots_tests`] the clear-then-record sequence around packaging.
//!
//! [`read_unlocked`]: super::record::read_unlocked
//! [`PackagingLease`]: super::lease::PackagingLease

use crate::boundary::package_set::record::{COMPLETION_FILE_NAME, read_unlocked};
use crate::boundary::package_set::scratch::{Scratch, scratch_root};
use anyhow::{Context, Result, ensure};
use cap_std::fs_utf8::Dir;
use rstest::rstest;

/// The run identifier the record under test names.
const THIS_RUN: &str = "record-regression-this-run";

/// Asking whether the record names this run does not change it.
///
/// A query that rewrote or removed the record could invalidate one another
/// process is about to reuse, and the damage would surface later as an
/// unexplained double pack rather than here. This compares the target's whole
/// state around the call, so a write, rename or removal all fail it.
#[rstest]
fn the_record_query_leaves_the_target_unchanged(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let directory = scratch.open()?;
    directory
        .write(COMPLETION_FILE_NAME, THIS_RUN)
        .context("plant a record naming this run")?;
    let before = listing(&directory)?;

    let reused = read_unlocked(&directory, Some(THIS_RUN))?;
    ensure!(reused, "a record naming this run read as not reusable");

    let after = listing(&directory)?;
    ensure!(
        before == after,
        "the record query changed the packaging target: {before:?} became \
         {after:?}; a read that writes can invalidate a record another process \
         is about to reuse"
    );
    Ok(())
}

/// A record naming no run is reusable by nobody, whatever it holds.
///
/// The empty-scope case is the single-process runners, where a process-local
/// cache is what deduplicates and a record left by an earlier run must not be
/// trusted. It is asserted here rather than folded into the case above because
/// it is the one answer that must be `false` even when the file reads cleanly.
#[rstest]
fn a_record_naming_no_run_is_not_reused(scratch_root: Result<Scratch>) -> Result<()> {
    let scratch = scratch_root?;
    let directory = scratch.open()?;
    directory
        .write(COMPLETION_FILE_NAME, THIS_RUN)
        .context("plant a record naming this run")?;

    ensure!(
        !read_unlocked(&directory, None)?,
        "a record was reused by a caller with no run identifier; without one the \
         process-local cache is what deduplicates, and a record from an earlier \
         run describes roots this run has not checked"
    );
    Ok(())
}

/// Every entry in `directory`, with its size, as a comparable snapshot.
///
/// Sizes rather than contents: this pins that a query did not write, rename or
/// remove anything, which is all the property claims, and a content hash would
/// restate the file read under test without adding to the comparison.
fn listing(directory: &Dir) -> Result<Vec<(String, u64)>> {
    let mut entries = Vec::new();
    for listed in directory.entries().context("list the packaging target")? {
        let entry = listed.context("read an entry of the packaging target")?;
        let name = entry
            .file_name()
            .context("read a file name of the packaging target")?;
        let metadata = entry
            .metadata()
            .with_context(|| format!("read the metadata of {name}"))?;
        entries.push((name, metadata.len()));
    }
    entries.sort();
    Ok(entries)
}
