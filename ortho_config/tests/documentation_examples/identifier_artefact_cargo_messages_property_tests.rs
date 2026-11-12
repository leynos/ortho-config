//! The Cargo JSON parser must read the same stream however it is chunked.
//!
//! Cargo writes its JSON messages to a pipe, and the observer receives whatever
//! bytes that pipe delivered: a record can be split at any byte, and two
//! records can arrive in one read. The parser therefore buffers until a
//! newline. These properties drive arbitrary record sequences through
//! arbitrary chunk boundaries and require the result to match both the whole
//! stream and an independently computed model, so a parser that only happened
//! to work for the chunking one run observed would be caught here.

use super::cargo_messages::{BuildCounts, CargoMessages, MAX_CARGO_RECORD_BYTES};
use proptest::prelude::*;
use std::path::PathBuf;

/// The fixture package, as Cargo prints it in `package_id`.
const FIXTURE: &str = "path+file:///fixture#orthohelp_fixture@0.1.0";

/// A dependency package, whose rebuilt units must not count as the fixture's.
const OTHER: &str = "path+file:///other#serde@1.0.0";

/// The build-script directories a generated fixture record may name.
const OUT_DIRS: [&str; 2] = ["/tmp/one", "/tmp/two"];

/// One record the generated stream may contain.
#[derive(Clone, Copy, Debug)]
enum Record {
    /// A compiler unit, fresh or recompiled, from the fixture or a dependency.
    Artefact { fresh: bool, fixture: bool },
    /// A fixture build-script report naming one of [`OUT_DIRS`].
    BuildScript(&'static str),
}

/// Generate streams that mix both record kinds in either ownership.
fn arb_record() -> impl Strategy<Value = Record> {
    prop_oneof![
        (any::<bool>(), any::<bool>())
            .prop_map(|(fresh, fixture)| Record::Artefact { fresh, fixture }),
        prop::sample::select(OUT_DIRS.to_vec()).prop_map(Record::BuildScript),
    ]
}

/// Render `records` as the newline-delimited stream Cargo would write.
fn render(records: &[Record]) -> Vec<u8> {
    let mut stream = String::new();
    for record in records {
        let message = match *record {
            Record::Artefact { fresh, fixture } => serde_json::json!({
                "reason": "compiler-artifact",
                "fresh": fresh,
                "package_id": if fixture { FIXTURE } else { OTHER },
            }),
            Record::BuildScript(directory) => serde_json::json!({
                "reason": "build-script-executed",
                "package_id": FIXTURE,
                "out_dir": directory,
            }),
        };
        stream.push_str(&message.to_string());
        stream.push('\n');
    }
    stream.into_bytes()
}

/// Feed `stream` to `messages` in the sizes given, then one byte at a time.
fn feed_in_chunks(messages: &mut CargoMessages, stream: &[u8], sizes: &[usize]) {
    for chunk in chunks(stream, sizes) {
        messages.observe(chunk);
    }
}

/// The byte ranges `sizes` describe over a stream of `stream_len` bytes.
///
/// Each size extends the previous end, clamped to the stream, so a size that
/// runs past the end names the remainder rather than a range outside it. Any
/// stream left after the generated sizes is split into single bytes, so every
/// generated case exercises a boundary that does not line up with the records.
/// The later sizes are unused once the stream ends, which is why the list may be
/// longer than the stream.
fn chunk_bounds(stream_len: usize, sizes: &[usize]) -> Vec<(usize, usize)> {
    let (mut bounds, offset) = sizes.iter().fold(
        (Vec::new(), 0usize),
        |(mut bounds, offset): (Vec<(usize, usize)>, usize), size| {
            let end = (offset + size).min(stream_len);
            bounds.push((offset, end));
            (bounds, end)
        },
    );
    bounds.extend((offset..stream_len).map(|index| (index, index + 1)));
    bounds
}

/// The chunks of `stream` those ranges name, in order.
///
/// `get` rather than an index expression keeps a malformed range from
/// panicking; an empty range yields an empty chunk, which the parser ignores.
fn chunks<'a>(stream: &'a [u8], sizes: &[usize]) -> Vec<&'a [u8]> {
    chunk_bounds(stream.len(), sizes)
        .into_iter()
        .filter_map(|(start, end)| stream.get(start..end))
        .collect()
}

/// The counts and directory a correct parser must produce, if any.
///
/// Two fixture build-script records are a rejection, matching the parser's own
/// rule: a single build produces exactly one script for the fixture package.
fn expected(records: &[Record]) -> Option<(BuildCounts, Option<PathBuf>)> {
    let mut scripts = records.iter().filter_map(|record| match *record {
        Record::BuildScript(directory) => Some(directory),
        Record::Artefact { .. } => None,
    });
    let script = scripts.next();
    if scripts.next().is_some() {
        return None;
    }
    let mut fresh = 0;
    let mut rebuilt = 0;
    let mut fixture_rebuilt = 0;
    for record in records {
        let Record::Artefact {
            fresh: was_fresh,
            fixture,
        } = *record
        else {
            continue;
        };
        if was_fresh {
            fresh += 1;
        } else {
            rebuilt += 1;
            if fixture {
                fixture_rebuilt += 1;
            }
        }
    }
    let out_dir = script.map(PathBuf::from);
    Some((BuildCounts::new(fresh, rebuilt, fixture_rebuilt), out_dir))
}

proptest! {
    /// No chunk boundary changes what the stream means.
    #[test]
    fn arbitrary_chunking_never_changes_the_parsed_stream(
        records in prop::collection::vec(arb_record(), 0..10),
        sizes in prop::collection::vec(1usize..17, 0..12),
    ) {
        let stream = render(&records);
        let mut whole = CargoMessages::for_package(FIXTURE.to_owned());
        whole.observe(&stream);
        let whole_result = whole.finish();
        let mut chunked = CargoMessages::for_package(FIXTURE.to_owned());
        feed_in_chunks(&mut chunked, &stream, &sizes);
        let chunked_result = chunked.finish();

        match (&whole_result, &chunked_result) {
            (Ok(whole_dir), Ok(chunked_dir)) => prop_assert_eq!(whole_dir, chunked_dir),
            (Err(whole_error), Err(chunked_error)) =>
                prop_assert_eq!(whole_error.to_string(), chunked_error.to_string()),
            _ => prop_assert!(false, "chunking changed whether the stream parsed"),
        }
        prop_assert_eq!(whole.counts(), chunked.counts());
    }

    /// A record over the bound stops parsing and keeps the first error.
    ///
    /// The over-long prefix is never terminated, so it cannot be mistaken for
    /// a record; the bytes after it include a well-formed artefact that must
    /// not be counted. Chunking varies so the rejection cannot depend on the
    /// over-long bytes arriving in one observation.
    #[test]
    fn an_over_long_record_is_rejected_before_any_later_record(
        overflow in 0usize..64,
        chunk_size in 1usize..(MAX_CARGO_RECORD_BYTES + 2),
    ) {
        let mut stream = vec![b' '; MAX_CARGO_RECORD_BYTES + 1 + overflow];
        stream.extend_from_slice(
            b"{\"reason\":\"compiler-artifact\",\"fresh\":true,\"package_id\":\"x\"}\n",
        );
        let mut messages = CargoMessages::for_package(FIXTURE.to_owned());
        for chunk in stream.chunks(chunk_size) {
            messages.observe(chunk);
        }
        let error = messages.finish().err();
        prop_assert!(
            error
                .as_ref()
                .is_some_and(|rejection| rejection.to_string().contains("128 KiB")),
            "the over-long record should be rejected with its bound: {:?}",
            error,
        );
        prop_assert_eq!(messages.counts(), BuildCounts::default());
    }

    /// The parser agrees with the model, so chunking is not the only check.
    #[test]
    fn the_parsed_counts_match_the_generated_model(
        records in prop::collection::vec(arb_record(), 0..10),
        sizes in prop::collection::vec(1usize..17, 0..12),
    ) {
        let stream = render(&records);
        let mut messages = CargoMessages::for_package(FIXTURE.to_owned());
        feed_in_chunks(&mut messages, &stream, &sizes);
        let result = messages.finish();

        match expected(&records) {
            Some((counts, out_dir)) => {
                prop_assert!(result.is_ok(), "a well-formed stream should parse: {:?}", result);
                prop_assert_eq!(messages.counts(), counts);
                prop_assert_eq!(result.unwrap_or_default(), out_dir);
            }
            None => prop_assert!(result.is_err(), "duplicate build-script records should be rejected"),
        }
    }
}
