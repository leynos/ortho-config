//! Check capture head, tail, and truncation invariants over arbitrary byte streams.
//!
//! The generated lengths deliberately straddle [`DIAGNOSTIC_TAIL_BYTES`]: below
//! it the tail is the whole stream, at it the window is exactly full, and above
//! it every earlier byte must be evicted. A capture that kept only the head, or
//! that stopped retaining once the tail was full, would still satisfy every
//! small-input case, so the sizes here include the window boundary and several
//! multiples of it. Chunk sizes reach past the window too, because one read can
//! deliver more than the tail retains.

use proptest::prelude::*;

use super::{DIAGNOSTIC_TAIL_BYTES, Progress};

/// Halfway through the smallest stream that exercises eviction.
///
/// `output_limit` stays inside this so the head is never larger than the
/// smallest generated stream, which keeps the "truncated" assertion meaningful
/// at every length rather than only at the largest.
const HEAD_LIMIT_SPAN: usize = 1024;

/// Half the retained window, as a shift because the gate refuses integer
/// division. A stream this long is retained whole, so it separates "below the
/// window" from "evicting".
const HALF_TAIL: usize = DIAGNOSTIC_TAIL_BYTES >> 1;

/// Stream lengths that straddle the retained window.
///
/// Below it the tail is the whole stream, at it the window is exactly full, and
/// above it every earlier byte must be evicted.
const LENGTHS: [usize; 8] = [
    0,
    1,
    HALF_TAIL,
    DIAGNOSTIC_TAIL_BYTES - 1,
    DIAGNOSTIC_TAIL_BYTES,
    DIAGNOSTIC_TAIL_BYTES + 1,
    2 * DIAGNOSTIC_TAIL_BYTES,
    3 * DIAGNOSTIC_TAIL_BYTES,
];

proptest! {
    #[test]
    fn capture_progress_matches_the_input_ends_for_every_chunking(
        // The length is chosen first and the bytes generated at that length:
        // a single `#[test]` signature builds its strategies as one tuple, so
        // a later binding cannot read an earlier one without `prop_flat_map`.
        input in prop::sample::select(LENGTHS.to_vec())
            .prop_flat_map(|length| prop::collection::vec(any::<u8>(), length..=length)),
        output_limit in 0usize..HEAD_LIMIT_SPAN,
        chunk_size in prop::sample::select(vec![
            1,
            7,
            128,
            8192,
            DIAGNOSTIC_TAIL_BYTES,
            DIAGNOSTIC_TAIL_BYTES + 1,
        ]),
    ) {
        let mut progress = Progress::default();
        for chunk in input.chunks(chunk_size) {
            progress.append(chunk, output_limit);
        }

        let snapshot = progress.snapshot();
        let captured_length = input.len().min(output_limit);
        prop_assert_eq!(
            snapshot.bytes.as_slice(),
            input.get(..captured_length).unwrap_or_default()
        );
        let tail_length = input.len().min(DIAGNOSTIC_TAIL_BYTES);
        prop_assert_eq!(
            snapshot.tail.as_slice(),
            input.get(input.len() - tail_length..).unwrap_or_default()
        );
        prop_assert_eq!(snapshot.bytes_seen, input.len() as u64);
        prop_assert_eq!(snapshot.truncated, input.len() > output_limit);
    }
}
