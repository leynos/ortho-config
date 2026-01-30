//! Step definitions for `cargo-orthohelp` behavioural tests.

use std::io::Read;
use std::process::Command;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, DirEntry};
use rstest::fixture;
use rstest_bdd_macros::{given, then, when};
use serde_json::Value;
use tempfile::TempDir;
use std::time::{Duration, SystemTime};

struct Harness {
    workspace_root: Utf8PathBuf,
    out_dir: TempDir,
    last_output: Option<std::process::Output>,
    cache_ir_path: Option<Utf8PathBuf>,
    cache_ir_mtime: Option<SystemTime>,
}

#[fixture]
fn harness() -> Harness {
    let manifest_dir = Utf8PathBuf::from_path_buf(std::path::PathBuf::from(
        env!("CARGO_MANIFEST_DIR"),
    ))
    .expect("manifest dir is UTF-8");
    let workspace_root = manifest_dir
        .parent()
        .expect("workspace root exists")
        .to_path_buf();
    Harness {
        workspace_root,
        out_dir: tempfile::tempdir().expect("create temp output dir"),
        last_output: None,
        cache_ir_path: None,
        cache_ir_mtime: None,
    }
}

#[given("a temporary output directory")]
fn temp_output_dir(harness: &mut Harness) {
    let path = Utf8PathBuf::from_path_buf(harness.out_dir.path().to_path_buf())
        .expect("temp output dir is UTF-8");
    let dir = Dir::open_ambient_dir(&path, ambient_authority())
        .expect("output dir should exist");
    let entries = dir.read_dir(".").expect("read output dir");
    assert_eq!(entries.count(), 0, "output dir should start empty");
}

#[given("the orthohelp cache is empty")]
fn cache_is_empty(harness: &mut Harness) {
    let root_dir = Dir::open_ambient_dir(harness.workspace_root.as_path(), ambient_authority())
        .expect("open workspace root");
    match root_dir.remove_dir_all("target/orthohelp") {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("remove orthohelp cache failed: {err}"),
    }
    harness.cache_ir_path = None;
    harness.cache_ir_mtime = None;
}

#[when("I run cargo-orthohelp with cache for the fixture")]
fn run_with_cache(harness: &mut Harness) {
    let output = run_orthohelp(
        harness,
        &["--cache", "--package", "orthohelp_fixture", "--locale", "en-US", "--locale", "fr-FR"],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    harness.last_output = Some(output);
    record_cache_state(harness);
}

#[when("I rerun cargo-orthohelp with cache for the fixture")]
fn rerun_with_cache(harness: &mut Harness) {
    // Ensure filesystem timestamp granularity distinguishes the cache file mtime.
    std::thread::sleep(Duration::from_secs(1));
    let output = run_orthohelp(
        harness,
        &["--cache", "--package", "orthohelp_fixture", "--locale", "en-US", "--locale", "fr-FR"],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    harness.last_output = Some(output);
}

#[when("I run cargo-orthohelp with no-build for the fixture")]
fn run_with_no_build(harness: &mut Harness) {
    let output = run_orthohelp(
        harness,
        &["--no-build", "--package", "orthohelp_fixture", "--locale", "en-US"],
    );
    harness.last_output = Some(output);
}

#[then("the output contains localized IR JSON for {locale}")]
fn output_contains_locale(harness: &mut Harness, locale: String) {
    let output = harness
        .last_output
        .as_ref()
        .expect("command output should be captured");
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    let out_root = Utf8PathBuf::from_path_buf(harness.out_dir.path().to_path_buf())
        .expect("output dir is UTF-8");
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())
        .expect("open output dir");
    let mut file = dir
        .open(&Utf8PathBuf::from(format!("ir/{locale}.json")))
        .expect("IR file exists");

    let mut buffer = String::new();
    file.read_to_string(&mut buffer)
        .expect("read IR JSON");

    let json: Value = serde_json::from_str(&buffer).expect("parse IR JSON");
    let ir_version = json
        .get("ir_version")
        .and_then(Value::as_str)
        .expect("ir_version field present");
    assert_eq!(
        ir_version,
        ortho_config::docs::ORTHO_DOCS_IR_VERSION,
        "IR version should match schema"
    );
    let json_locale = json
        .get("locale")
        .and_then(Value::as_str)
        .expect("locale field present");
    assert_eq!(json_locale, locale);
    let about = json
        .get("about")
        .and_then(Value::as_str)
        .expect("about field present");
    assert_eq!(about, expected_about(&locale));

    let help = json
        .get("fields")
        .and_then(Value::as_array)
        .and_then(|fields| fields.first())
        .and_then(|field| field.get("help"))
        .and_then(Value::as_str)
        .expect("field help present");
    assert_eq!(help, expected_help(&locale));
}

#[then("the cached IR is reused")]
fn cached_ir_reused(harness: &mut Harness) {
    let cache_path = harness
        .cache_ir_path
        .as_ref()
        .expect("cached IR path should be recorded");
    let previous = harness
        .cache_ir_mtime
        .expect("cached IR timestamp should be recorded");
    let metadata = std::fs::metadata(cache_path.as_std_path())
        .expect("cached IR metadata should be available");
    let current = metadata.modified().expect("cached IR mtime should exist");
    assert_eq!(previous, current, "cached IR should not be rewritten");
}

#[then("the cached IR deserializes into the schema")]
fn cached_ir_deserializes(harness: &mut Harness) {
    let cache_path = harness
        .cache_ir_path
        .as_ref()
        .expect("cached IR path should be recorded");
    let json = std::fs::read_to_string(cache_path.as_std_path())
        .expect("cached IR should be readable");
    let metadata: ortho_config::docs::DocMetadata =
        serde_json::from_str(&json).expect("cached IR should deserialize");
    assert_eq!(
        metadata.ir_version,
        ortho_config::docs::ORTHO_DOCS_IR_VERSION,
        "cached IR should match the current schema version"
    );
}

#[then("the command fails due to missing cache")]
fn command_fails_due_to_missing_cache(harness: &mut Harness) {
    let output = harness
        .last_output
        .as_ref()
        .expect("command output should be captured");
    assert!(!output.status.success(), "cargo-orthohelp should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cached IR missing"),
        "expected missing cache error, got: {stderr}"
    );
}

fn run_orthohelp(harness: &Harness, args: &[&str]) -> std::process::Output {
    let exe = cargo_orthohelp_exe();
    let mut command = Command::new(exe.as_str());
    command
        .current_dir(harness.workspace_root.as_str())
        .arg("--out-dir")
        .arg(harness.out_dir.path())
        .args(args);
    command.output().expect("run cargo-orthohelp")
}

fn cargo_orthohelp_exe() -> Utf8PathBuf {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_cargo-orthohelp") {
        return Utf8PathBuf::from(path);
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_cargo_orthohelp") {
        return Utf8PathBuf::from(path);
    }
    panic!("cargo-orthohelp binary path not found in environment");
}

fn record_cache_state(harness: &mut Harness) {
    let cache_path = find_cached_ir(harness).expect("cached IR should exist");
    let metadata = std::fs::metadata(cache_path.as_std_path())
        .expect("cached IR metadata should be available");
    let modified = metadata.modified().expect("cached IR mtime should exist");
    harness.cache_ir_path = Some(cache_path);
    harness.cache_ir_mtime = Some(modified);
}

fn find_cached_ir(harness: &Harness) -> Option<Utf8PathBuf> {
    let cache_root = harness
        .workspace_root
        .join("target")
        .join("orthohelp");
    let dir = Dir::open_ambient_dir(&cache_root, ambient_authority()).ok()?;
    let mut newest: Option<(SystemTime, Utf8PathBuf)> = None;
    for entry in dir.read_dir(".").ok()? {
        if let Some(candidate) = check_cache_entry(&cache_root, entry, &newest) {
            newest = Some(candidate);
        }
    }
    newest.map(|(_, path)| path)
}

fn check_cache_entry(
    cache_root: &Utf8PathBuf,
    entry: Result<DirEntry, std::io::Error>,
    newest: &Option<(SystemTime, Utf8PathBuf)>,
) -> Option<(SystemTime, Utf8PathBuf)> {
    let entry = entry.ok()?;
    let file_type = entry.file_type().ok()?;
    if !file_type.is_dir() {
        return None;
    }

    let file_name = entry.file_name().ok()?;
    let ir_path = cache_root.join(Utf8PathBuf::from(file_name)).join("ir.json");
    if !ir_path.exists() {
        return None;
    }

    let metadata = std::fs::metadata(ir_path.as_std_path()).ok()?;
    let modified = metadata.modified().ok()?;
    let replace = newest
        .as_ref()
        .map_or(true, |(best_time, _)| modified > *best_time);
    if !replace {
        return None;
    }

    Some((modified, ir_path))
}

fn expected_about(locale: &str) -> &'static str {
    match locale {
        "fr-FR" => "Configuration du fixture Orthohelp.",
        _ => "Orthohelp fixture configuration.",
    }
}

fn expected_help(locale: &str) -> &'static str {
    match locale {
        "fr-FR" => "Port utilisé par le service de test.",
        _ => "Port used by the fixture service.",
    }
}
