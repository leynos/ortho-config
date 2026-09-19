//! Unit tests for atomic JSON artefact writing.

use super::*;
use crate::policy::PolicyMode;
use camino::Utf8Path;
use std::collections::HashSet;
use std::sync::Arc;
use tempfile::TempDir;

#[test]
fn policy_report_round_trips_through_its_writer() {
    let temp_dir = TempDir::new().expect("create temporary output directory");
    let out_dir = Utf8Path::from_path(temp_dir.path()).expect("temporary path is UTF-8");
    let report = PolicyReport::empty(PolicyMode::Warn);

    let path = write_policy_report(out_dir, &report).expect("write policy report");
    let content = std::fs::read_to_string(&path).expect("read policy report");
    let persisted =
        serde_json::from_str::<PolicyReport>(&content).expect("deserialize policy report");

    assert_eq!(persisted, report);
}

#[test]
fn concurrent_writes_do_not_corrupt_output() {
    let temp_dir = TempDir::new().expect("create temporary output directory");
    let out_dir = Utf8Path::from_path(temp_dir.path()).expect("temporary path is UTF-8");
    let payload = Arc::new(AgentContext::new("test-package"));

    let handles = (0..8)
        .map(|_| {
            let thread_payload = Arc::clone(&payload);
            let thread_out_dir = out_dir.to_path_buf();
            std::thread::spawn(move || write_agent_context(&thread_out_dir, &thread_payload))
        })
        .collect::<Vec<_>>();

    for handle in handles {
        let result = handle.join().expect("thread panicked");
        assert!(result.is_ok(), "write_agent_context failed: {result:?}");
    }

    let content =
        std::fs::read_to_string(out_dir.join("agent-context.json")).expect("read output JSON");
    serde_json::from_str::<serde_json::Value>(&content).expect("parse output JSON");
}

#[test]
fn temp_file_collision_fails_hard() {
    let temp_dir = TempDir::new().expect("create temporary output directory");
    let out_dir = Utf8Path::from_path(temp_dir.path()).expect("temporary path is UTF-8");
    let dir = ensure_dir(out_dir).expect("open output directory");
    let target = JsonArtefactWriteTarget::new(out_dir, "agent-context.json");

    let _first = open_json_temp_file(&dir, &target, "agent-context")
        .expect("first temp file creation should succeed");

    let second = open_json_temp_file(&dir, &target, "agent-context");
    assert!(
        matches!(
            &second,
            Err(OrthohelpError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists
        ),
        "expected create_new collision to report AlreadyExists, got {second:?}"
    );
}

#[test]
fn temp_file_open_fails_when_file_already_exists() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let out_dir = Utf8Path::from_path(temp_dir.path()).expect("path is UTF-8");
    let dir = Dir::open_ambient_dir(out_dir, ambient_authority()).expect("open temp dir");

    std::fs::File::create(out_dir.join("collision.tmp")).expect("pre-create collision file");
    let target = JsonArtefactWriteTarget {
        filename: "agent-context.json",
        path: out_dir.join("agent-context.json"),
        temp_filename: "collision.tmp".to_owned(),
        temp_path: out_dir.join("collision.tmp"),
    };

    let result = open_json_temp_file(&dir, &target, "agent-context");
    assert!(
        matches!(
            &result,
            Err(OrthohelpError::Io { source, .. })
                if source.kind() == std::io::ErrorKind::AlreadyExists
        ),
        "expected create_new collision to report AlreadyExists, got {result:?}"
    );
}

#[test]
fn concurrent_writes_produce_unique_temp_names() {
    let temp_dir = TempDir::new().expect("create temporary output directory");
    let out_dir = Utf8Path::from_path(temp_dir.path()).expect("temporary path is UTF-8");

    let temp_filenames = (0..8)
        .map(|_| JsonArtefactWriteTarget::new(out_dir, "agent-context.json").temp_filename)
        .collect::<Vec<_>>();
    let unique_temp_filenames = temp_filenames.iter().collect::<HashSet<_>>();

    assert_eq!(unique_temp_filenames.len(), temp_filenames.len());
}
