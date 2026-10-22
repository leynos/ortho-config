//! Property coverage for Cargo JSON chunking and record-size boundaries.

use super::super::{MAX_CARGO_RECORD_BYTES, binary_file_name, read_cargo_messages};
use super::{
    TARGET_NAME, artifact_record, encode_records, generated_target, generated_target_directory,
};
use anyhow::{Context, Result, ensure};
use proptest::prelude::*;
use serde_json::json;
use std::io::{self, BufRead, Cursor, Read};

proptest! {
    #[test]
    fn cargo_protocol_results_are_independent_of_reader_chunking(
        chunk_sizes in prop::collection::vec(1usize..128, 1..64),
    ) {
        let target_directory = generated_target_directory().expect("create target identity");
        let target = generated_target(&target_directory);
        let executable = target_directory
            .join("debug")
            .join(binary_file_name(TARGET_NAME))
            .to_string();
        let dep_info = target_directory
            .join("debug")
            .join(format!("{TARGET_NAME}.d"))
            .to_string();
        let hashed = target_directory
            .join("debug")
            .join("deps")
            .join(format!("{TARGET_NAME}-fixture-hash"))
            .to_string();
        let records = [
            json!({
                "reason": "compiler-message",
                "message": { "rendered": "first diagnostic\n" },
            }),
            artifact_record(
                TARGET_NAME,
                &[executable.clone(), dep_info, hashed],
                Some(&executable),
            ),
            artifact_record(
                TARGET_NAME,
                std::slice::from_ref(&executable),
                Some(&executable),
            ),
            artifact_record(TARGET_NAME, &["relative-path".to_owned()], Some("relative-path")),
            {
                let unexpected_path = target_directory.join("debug/unexpected");
                artifact_record(
                    TARGET_NAME,
                    &[unexpected_path.to_string()],
                    Some(unexpected_path.as_str()),
                )
            },
            json!({
                "reason": "compiler-message",
                "message": { "rendered": "second diagnostic\n" },
            }),
        ];
        let stream = encode_records(&records).expect("encode Cargo records");
        let mut complete_reader = Cursor::new(stream.clone());
        let expected = read_cargo_messages(&mut complete_reader, &target)
            .expect("read complete Cargo stream");
        let mut fragmented_reader = ChunkedReader::new(stream, chunk_sizes);

        let actual = read_cargo_messages(&mut fragmented_reader, &target)
            .expect("read chunked Cargo stream");

        prop_assert_eq!(actual.executable, expected.executable);
        prop_assert_eq!(actual.generated_outputs, expected.generated_outputs);
        prop_assert_eq!(actual.diagnostics.as_str(), "first diagnostic\nsecond diagnostic\n");
        prop_assert_eq!(actual.diagnostics.as_str(), expected.diagnostics.as_str());
        prop_assert_eq!(actual.diagnostics_truncated, expected.diagnostics_truncated);
        prop_assert_eq!(
            actual.protocol_error.as_ref().map(ToString::to_string),
            expected.protocol_error.as_ref().map(ToString::to_string),
        );
        prop_assert!(
            actual.protocol_error
                .as_ref()
                .is_some_and(|error| error.to_string().contains("path was not absolute")),
            "later protocol errors replaced the first error"
        );
    }
}

#[test]
fn cargo_record_limit_accepts_exactly_128_kib_and_rejects_more() -> Result<()> {
    let target_directory = generated_target_directory()?;
    let target = generated_target(&target_directory);
    let base = json!({"reason":"build-script-executed", "padding":""}).to_string();
    let padding_length = MAX_CARGO_RECORD_BYTES - 1 - base.len();
    let exact_record = json!({
        "reason": "build-script-executed",
        "padding": "x".repeat(padding_length),
    })
    .to_string();
    ensure!(exact_record.len() + 1 == MAX_CARGO_RECORD_BYTES);
    let mut exact_stream = exact_record.into_bytes();
    exact_stream.push(b'\n');
    let mut exact_reader = Cursor::new(exact_stream);

    let exact = read_cargo_messages(&mut exact_reader, &target)?;

    ensure!(exact.protocol_error.is_none());
    let oversized_record = json!({
        "reason": "build-script-executed",
        "padding": "x".repeat(padding_length + 1),
    })
    .to_string();
    let mut oversized_stream = oversized_record.into_bytes();
    oversized_stream.push(b'\n');
    let mut oversized_reader = Cursor::new(oversized_stream);

    let oversized = read_cargo_messages(&mut oversized_reader, &target)?;

    let error = oversized
        .protocol_error
        .context("expected the oversized Cargo record to be rejected")?;
    ensure!(error.to_string().contains("exceeded 128 KiB"));
    Ok(())
}

struct ChunkedReader {
    bytes: Vec<u8>,
    offset: usize,
    chunk_sizes: Vec<usize>,
    chunk_index: usize,
}

impl ChunkedReader {
    fn new(bytes: Vec<u8>, chunk_sizes: Vec<usize>) -> Self {
        Self {
            bytes,
            offset: 0,
            chunk_sizes,
            chunk_index: 0,
        }
    }
}

impl Read for ChunkedReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let chunk_size = self.current_chunk_size();
        let end = self.offset.saturating_add(chunk_size).min(self.bytes.len());
        let source = self.bytes.get(self.offset..end).unwrap_or_default();
        let count = source.len().min(output.len());
        output
            .get_mut(..count)
            .unwrap_or_default()
            .copy_from_slice(source.get(..count).unwrap_or_default());
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for ChunkedReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let chunk_size = self.current_chunk_size();
        let end = self.offset.saturating_add(chunk_size).min(self.bytes.len());
        Ok(self.bytes.get(self.offset..end).unwrap_or_default())
    }

    fn consume(&mut self, amount: usize) {
        let remaining = self.bytes.len().saturating_sub(self.offset);
        let consumed = amount.min(remaining);
        self.offset += consumed;
        if consumed > 0 {
            let next_index = self.chunk_index.saturating_add(1);
            self.chunk_index = if next_index >= self.chunk_sizes.len() {
                0
            } else {
                next_index
            };
        }
    }
}

impl ChunkedReader {
    fn current_chunk_size(&self) -> usize {
        self.chunk_sizes.get(self.chunk_index).copied().unwrap_or(1)
    }
}
