//! Step definitions for `cargo-orthohelp` behavioural tests.

use std::io::Read;
use std::process::Command;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest::fixture;
use rstest_bdd_macros::{given, then, when};
use serde_json::Value;
use tempfile::TempDir;

struct Harness {
    workspace_root: Utf8PathBuf,
    out_dir: TempDir,
    last_output: Option<std::process::Output>,
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

#[when("I run cargo-orthohelp with cache for the fixture")]
fn run_with_cache(harness: &mut Harness) {
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
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    harness.last_output = Some(output);
}

#[then("the output contains localised IR JSON for {locale}")]
fn output_contains_locale(harness: &mut Harness, locale: String) {
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
