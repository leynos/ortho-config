//! Cargo JSON extraction and private snapshot regression tests.

use super::{Artifacts, MAX_MESSAGE_BYTES};
use anyhow::{Result, ensure};
use cap_std::{ambient_authority, fs::Dir};

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
