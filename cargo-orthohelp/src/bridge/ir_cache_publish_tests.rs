//! Publish-path tests for the bounded IR cache read.
//!
//! The seam tests in `bounded_tests` drive the comparison directly. This test
//! publishes through `write_ir_cache` and counts every byte read from the
//! opened cache entry, so a full read added between the open and the
//! comparison — for example in `cached_ir_matches` — is caught as well.

use rstest::rstest;

use super::bounded_tests::{CacheFixture, cache_fixture};
use super::read_observation;
use super::{PublishOutcome, write_ir_cache};

/// A size-mismatched entry is replaced without any publish-path body read.
///
/// The entry is larger than the payload, so the metadata check rejects it
/// before the comparison reads anything. The observed count covers the whole
/// publish path: the opened handle is wrapped in `CacheEntry` before
/// `cached_ir_matches` can use it, and its bytes are reachable only through
/// the counting reader.
#[rstest]
#[expect(
    clippy::panic_in_result_fn,
    reason = "Assertions report publish-path read mismatches more clearly than errors"
)]
fn write_ir_cache_reads_no_body_from_a_size_mismatched_entry(
    cache_fixture: Result<CacheFixture, Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    const PAYLOAD: &str = r#"{"ir_version":"1.0.0"}"#;

    let fixture = cache_fixture?;
    let larger_entry = "x".repeat(PAYLOAD.len().saturating_add(64));
    fixture.dir.write("ir.json", &larger_entry)?;
    read_observation::reset();

    assert_eq!(
        write_ir_cache(&fixture.paths, PAYLOAD).expect("replace the mismatched entry"),
        PublishOutcome::Updated,
        "a size-mismatched cache entry must be replaced"
    );
    assert_eq!(
        read_observation::observed(),
        0,
        "the publish path must not read the body of a size-mismatched entry"
    );
    Ok(())
}
