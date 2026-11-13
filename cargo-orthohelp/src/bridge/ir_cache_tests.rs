//! Check the cache's unreadable-entry and unreplaceable-target branches.
//!
//! The orchestration tests beside this module cover the outcomes a healthy
//! cache produces. What they cannot reach are the two failures an operator
//! does meet: an entry that exists but cannot be opened, and a publish target
//! that cannot be replaced. Both have to be reported with the stage that
//! produced them, because the caller names the outcome from that stage alone.

use super::BridgeCache;
use super::ir_cache::{PublishStage, write_ir_cache};
use crate::error::OrthohelpError;

use camino::Utf8PathBuf;
use std::error::Error;
use tempfile::TempDir;

/// IR payload for the write-stage case.
const CONTENT: &str = r#"{"ir_version":"1.0.0"}"#;

/// A cache rooted at `root`, publishing to `ir_path`.
fn cache_at(root: &TempDir, ir_path: Utf8PathBuf) -> Result<BridgeCache, Box<dyn Error>> {
    let cache_dir = Utf8PathBuf::from_path_buf(root.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    Ok(BridgeCache {
        target_dir: cache_dir.join("target"),
        cache_dir,
        ir_path,
    })
}

/// A cache rooted at `root` whose publish target is its own `ir.json`.
fn cache_in(root: &TempDir) -> Result<BridgeCache, Box<dyn Error>> {
    let cache = cache_at(root, Utf8PathBuf::new())?;
    let ir_path = cache.cache_dir.join("ir.json");
    Ok(BridgeCache { ir_path, ..cache })
}

/// The path the publish attempt was working on when it failed.
fn failed_path(source: OrthohelpError) -> Utf8PathBuf {
    match source {
        OrthohelpError::Io { path, .. } => path,
        other => panic!("a cache failure is an I/O error, not {other:?}"),
    }
}

#[cfg(unix)]
#[test]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions state the published stage more clearly than errors"
)]
fn an_entry_that_cannot_be_opened_is_a_read_failure() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let cache = cache_in(&root)?;
    // Opening this link loops, which fails with a kind that is not `NotFound`
    // on every account, including one privileged enough to read a mode-0000
    // file. That distinguishes "absent, so generate" from "present but
    // unreadable, so refuse to overwrite".
    std::os::unix::fs::symlink("ir.json", cache.ir_path.as_std_path())?;

    let error = write_ir_cache(&cache, CONTENT)
        .expect_err("an entry that cannot be opened must not be overwritten");

    assert_eq!(
        error.stage(),
        PublishStage::Read,
        "an unreadable entry is a read failure, not a write failure"
    );
    assert_eq!(
        failed_path(error.into_source()),
        cache.ir_path,
        "the failure names the cache entry it could not read"
    );
    Ok(())
}

#[test]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions state the published stage more clearly than errors"
)]
fn a_publish_target_that_cannot_be_replaced_is_a_write_failure() -> Result<(), Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    // The read step always consults the fixed `ir.json` name, so a directory
    // at the publish path leaves the read with a cache miss and reaches the
    // atomic replace, where renaming a file over a directory fails.
    let blocked = Utf8PathBuf::from_path_buf(root.path().join("blocked"))
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    std::fs::create_dir(&blocked)?;
    let cache = cache_at(&root, blocked)?;

    let error =
        write_ir_cache(&cache, CONTENT).expect_err("a directory cannot be replaced by a file");

    assert_eq!(
        error.stage(),
        PublishStage::Write,
        "a failed replace is a write failure, not a read failure"
    );
    assert_eq!(
        failed_path(error.into_source()),
        cache.ir_path,
        "the failure names the entry it could not publish"
    );
    Ok(())
}
