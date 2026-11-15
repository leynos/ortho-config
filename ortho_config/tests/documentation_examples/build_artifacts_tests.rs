//! Cargo JSON extraction and private snapshot regression tests.

use super::{Artifacts, MAX_MESSAGE_BYTES};
use anyhow::{Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
use proptest::prelude::*;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn executable_records_survive_chunk_boundaries_and_large_output() -> Result<()> {
    let source = tempfile::tempdir()?;
    let destination = tempfile::tempdir()?;
    let directory = Dir::open_ambient_dir(source.path(), ambient_authority())?;
    directory.write("same-name", b"owned executable")?;
    let artifact = serde_json::json!({
        "reason": "compiler-artifact", "target": {"name": "same-name", "kind": ["bin"]},
        "executable": source.path().join("same-name"),
    })
    .to_string();
    let mut artifacts = Artifacts::default();
    for _ in 0..50_000 {
        artifacts.consume(b"{\"reason\":\"build-script-executed\"}\n");
    }
    for byte in artifact.as_bytes() {
        artifacts.consume(&[*byte]);
    }
    artifacts.consume(b"\n");
    let copied = artifacts.snapshot(destination.path())?;
    ensure!(copied.get("same-name") == Some(&destination.path().join("same-name")));
    ensure!(
        Dir::open_ambient_dir(destination.path(), ambient_authority())?.read("same-name")?
            == b"owned executable"
    );
    Ok(())
}

#[test]
fn oversized_cargo_message_fails_closed() {
    let mut artifacts = Artifacts::default();
    artifacts.consume(&vec![b'x'; MAX_MESSAGE_BYTES + 1]);
    artifacts.consume(b"\n");
    let destination = tempfile::tempdir().expect("create snapshot directory");
    assert!(
        artifacts
            .snapshot(destination.path())
            .expect_err("oversized JSON must fail")
            .to_string()
            .contains("oversized")
    );
}

proptest! {
    #[test]
    fn artifact_records_are_independent_of_chunk_boundaries(
        target_ids in prop::collection::vec(0u8..5, 1..20),
        chunk_sizes in prop::collection::vec(1usize..256, 1..100),
    ) {
        let mut stream = Vec::new();
        let mut expected = BTreeMap::new();
        for (index, target_id) in target_ids.iter().enumerate() {
            append_record(&mut stream, &serde_json::json!({
                "reason": "compiler-message",
                "message": { "rendered": format!("diagnostic-{index}\n") },
            }));
            let name = format!("fixture-{target_id}");
            let executable = format!("/private/build/{index}/{name}");
            append_record(&mut stream, &serde_json::json!({
                "reason": "compiler-artifact",
                "target": { "name": name.clone(), "kind": ["bin"] },
                "executable": executable.clone(),
            }));
            expected.insert(name, PathBuf::from(executable));
        }

        let mut artifacts = Artifacts::default();
        let mut offset: usize = 0;
        for chunk_size in chunk_sizes {
            let end = offset.saturating_add(chunk_size).min(stream.len());
            artifacts.consume(stream.get(offset..end).unwrap_or_default());
            offset = end;
            if offset == stream.len() {
                break;
            }
        }
        if offset < stream.len() {
            artifacts.consume(stream.get(offset..).unwrap_or_default());
        }
        artifacts.finish_line();

        prop_assert!(!artifacts.invalid);
        prop_assert_eq!(artifacts.executables, expected);
    }
}

#[test]
fn cargo_json_record_limit_accepts_the_boundary_and_rejects_one_more_byte() {
    let base = serde_json::json!({
        "reason": "build-script-executed",
        "padding": "",
    })
    .to_string();
    let padding_length = MAX_MESSAGE_BYTES - base.len();
    let exact_record = serde_json::json!({
        "reason": "build-script-executed",
        "padding": "x".repeat(padding_length),
    })
    .to_string();
    assert_eq!(exact_record.len(), MAX_MESSAGE_BYTES);

    let mut at_limit = Artifacts::default();
    at_limit.consume(exact_record.as_bytes());
    at_limit.consume(b"\n");
    assert!(!at_limit.invalid);

    let oversized_record = serde_json::json!({
        "reason": "build-script-executed",
        "padding": "x".repeat(padding_length + 1),
    })
    .to_string();
    let mut over_limit = Artifacts::default();
    over_limit.consume(oversized_record.as_bytes());
    over_limit.consume(b"\n");
    assert!(over_limit.invalid);
}

fn append_record(stream: &mut Vec<u8>, record: &serde_json::Value) {
    stream.extend_from_slice(record.to_string().as_bytes());
    stream.push(b'\n');
}
