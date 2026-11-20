//! Property tests for the bounded IR cache comparison.
//!
//! The example tests in the sibling `bounded_tests` module pin the
//! comparison's contract at readable points: a length mismatch, an overgrown
//! body, and an exact match. These properties sweep the same contract over
//! payloads of arbitrary bytes, bodies built from those payloads in several
//! shapes, and reported lengths that may or may not agree, so an
//! implementation that special-cased the examples cannot pass.
//!
//! Every case drives `compare_cache_entry`, the comparison the publish path
//! performs, through the same `CountingReader` the example tests use. A zero
//! read count on a mismatch is therefore evidence about the publish path, not
//! about a helper it might bypass.

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use super::bounded_tests::CountingReader;
use super::compare_cache_entry;

/// Payloads paired with a body derived from that payload.
///
/// The shapes cover the distinctions a comparison must make: an exact copy, a
/// truncated copy, an extension whose prefix still matches, a single-byte edit
/// that keeps the length, and an unrelated body. An arbitrary body alone would
/// almost never reproduce the prefix and same-length shapes that separate an
/// exact comparison from a laxer one.
fn payload_and_body() -> impl Strategy<Value = (String, Vec<u8>)> {
    (
        any::<String>(),
        0usize..5,
        1u8..=u8::MAX,
        1usize..4,
        prop::collection::vec(any::<u8>(), 1..8),
    )
        .prop_map(|(json, shape, mask, shortfall, extra)| {
            let mut body = json.clone().into_bytes();
            match shape {
                0 => {}
                1 => {
                    let kept = body.len().saturating_sub(shortfall);
                    body.truncate(kept);
                }
                2 => body.extend_from_slice(&extra),
                3 => {
                    if let Some(byte) = body.first_mut() {
                        *byte ^= mask;
                    }
                }
                _ => body = extra,
            }
            (json, body)
        })
}

proptest! {
    /// A matching length reports exact equality; a mismatch reports `false`
    /// without touching the body; and no case consumes more than the payload
    /// plus one byte.
    ///
    /// The mismatch assertion is the publish-path guarantee: a cache file that
    /// reports any other size is rejected on metadata alone, so its contents
    /// are never loaded, however large they are.
    #[test]
    fn the_comparison_is_exact_bounded_and_lazy_on_mismatch(
        (json, body) in payload_and_body(),
        reported_len in any::<u64>(),
        report_true_length in any::<bool>(),
    ) {
        let true_len = json.len() as u64;
        let cached_len = if report_true_length {
            true_len
        } else if reported_len == true_len {
            reported_len.wrapping_add(1)
        } else {
            reported_len
        };

        let mut reader = CountingReader::new(&body);
        let matches = compare_cache_entry(&mut reader, cached_len, true, &json)
            .map_err(|error| TestCaseError::fail(format!("compare in-memory body: {error}")))?;

        prop_assert!(
            reader.consumed <= json.len().saturating_add(1),
            "comparison consumed {} bytes for a payload of {} bytes",
            reader.consumed,
            json.len()
        );

        if cached_len == true_len {
            prop_assert_eq!(
                matches,
                body.as_slice() == json.as_bytes(),
                "a matching length reported the wrong equality result for payload {:?}",
                json
            );
        } else {
            prop_assert!(!matches, "a length mismatch matched payload {:?}", json);
            prop_assert_eq!(
                reader.read_calls,
                0,
                "a length mismatch read the body of payload {:?}",
                json
            );
        }
    }
}
