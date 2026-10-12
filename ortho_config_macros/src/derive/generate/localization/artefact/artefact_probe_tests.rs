//! Operating-system-independent policy tests for fragment source probing.
//!
//! The stale-fragment policy — a missing source drops the fragment, every
//! other metadata failure aborts merging — must hold on every platform, but
//! no filesystem layout provokes the same metadata error portably: a regular
//! file in a parent position yields `NotADirectory` on Unix and `NotFound`
//! on Windows, and `ErrorKind::NotADirectory` has no Windows counterpart at
//! all. These tests therefore inject metadata outcomes and drive the shared
//! classification directly. The merge-level regression tests in
//! `artefact_fragment_tests.rs` exercise the same injection through the full
//! merge path.

use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use super::{source_still_exists, with_metadata_outcome};

static PROBE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Builds a unique scratch path for one probe case.
///
/// Injected-error cases never touch the filesystem, so the path only has to
/// be unique, not real.
fn scratch_path(label: &str) -> PathBuf {
    let sequence = PROBE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "ortho-config-probe-{label}-{}-{sequence}.rs",
        std::process::id()
    ))
}

/// Verifies only `NotFound` marks a fragment stale, and that every other
/// metadata error kind is propagated unchanged.
#[rstest]
#[case(ErrorKind::NotFound, Ok(false))]
#[case(ErrorKind::PermissionDenied, Err(ErrorKind::PermissionDenied))]
#[case(ErrorKind::NotADirectory, Err(ErrorKind::NotADirectory))]
#[case(ErrorKind::Other, Err(ErrorKind::Other))]
fn metadata_error_kinds_follow_the_staleness_policy(
    #[case] kind: ErrorKind,
    #[case] expected: Result<bool, ErrorKind>,
) -> Result<()> {
    let path = scratch_path("kind-policy");
    let classified = with_metadata_outcome(&path, Err(kind), || source_still_exists(&path))
        .map_err(|error| error.kind());
    ensure!(
        classified == expected,
        "injected {kind:?} was classified as {classified:?}"
    );
    Ok(())
}

/// Verifies a successful metadata outcome retains the fragment through the
/// shared classification path.
///
/// The injection records a real path and re-issues the real probe against
/// it, so the production wrapper, not a synthesized value, supplies the
/// metadata the classification reads.
#[test]
fn successful_metadata_retains_the_fragment() -> Result<()> {
    let path = scratch_path("success");
    fs::write(&path, "// probe fixture\n").context("write probe fixture")?;
    let retained = with_metadata_outcome(&path, Ok(()), || source_still_exists(&path));
    fs::remove_file(&path).context("remove probe fixture")?;
    let is_retained = matches!(&retained, Ok(true));
    ensure!(
        is_retained,
        "successful metadata must retain the fragment, got {retained:?}"
    );
    Ok(())
}
