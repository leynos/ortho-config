//! Check capture prefix and truncation invariants over arbitrary byte streams.

use proptest::prelude::*;

use super::Progress;

proptest! {
    #[test]
    fn capture_progress_matches_the_input_prefix_for_every_chunking(
        input in prop::collection::vec(any::<u8>(), 0..2048),
        output_limit in 0usize..1024,
        chunk_size in 1usize..128,
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
        prop_assert_eq!(snapshot.bytes_seen, input.len() as u64);
        prop_assert_eq!(snapshot.truncated, input.len() > output_limit);
    }
}
