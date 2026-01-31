//! Step definitions for `cargo-orthohelp` behavioural tests.

use std::io::Read;
use std::process::Command;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::{Dir, DirEntry};
use cap_std::time::SystemTime;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{given, then, when, ScenarioState};
use serde_json::Value;
use std::time::Duration;
use tempfile::TempDir;

/// Scenario state for cargo-orthohelp scenarios.
#[derive(Debug, Default, ScenarioState)]
pub struct OrthoHelpContext {
    pub workspace_root: Slot<Utf8PathBuf>,
    pub out_dir: Slot<TempDir>,
    pub last_output: Slot<std::process::Output>,
    pub cache_ir_path: Slot<Utf8PathBuf>,
    pub cache_ir_mtime: Slot<SystemTime>,
}

/// Provides a clean context for orthohelp scenarios.
#[fixture]
pub fn orthohelp_context() -> OrthoHelpContext {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .expect("workspace root exists")
        .to_path_buf();
    let ctx = OrthoHelpContext::default();
    ctx.workspace_root.set(workspace_root);
    ctx.out_dir.set(tempfile::tempdir().expect("create temp output dir"));
    ctx
}

fn get_out_dir(ctx: &OrthoHelpContext) -> Utf8PathBuf {
    ctx.out_dir
        .with_ref(|dir| Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).expect("UTF-8"))
        .expect("out_dir should be set")
}

fn get_workspace_root(ctx: &OrthoHelpContext) -> Utf8PathBuf {
    ctx.workspace_root
        .with_ref(|root| root.clone())
        .expect("workspace_root should be set")
}

#[given("a temporary output directory")]
fn temp_output_dir(orthohelp_context: &mut OrthoHelpContext) {
    let path = get_out_dir(orthohelp_context);
    let dir = Dir::open_ambient_dir(&path, ambient_authority())
        .expect("output dir should exist");
    let entries = dir.read_dir(".").expect("read output dir");
    assert_eq!(entries.count(), 0, "output dir should start empty");
}

#[given("the orthohelp cache is empty")]
fn cache_is_empty(orthohelp_context: &mut OrthoHelpContext) {
    let workspace_root = get_workspace_root(orthohelp_context);
    let root_dir = Dir::open_ambient_dir(workspace_root.as_path(), ambient_authority())
        .expect("open workspace root");
    if let Err(err) = root_dir.remove_dir_all("target/orthohelp") {
        if !is_not_found_kind(&err) {
            panic!("remove orthohelp cache failed: {err}");
        }
    }
    orthohelp_context.cache_ir_path.clear();
    orthohelp_context.cache_ir_mtime.clear();
}

fn is_not_found_kind(err: &std::io::Error) -> bool {
    matches!(err.kind(), std::io::ErrorKind::NotFound)
}

#[when("I run cargo-orthohelp with cache for the fixture")]
fn run_with_cache(orthohelp_context: &mut OrthoHelpContext) {
    let output = run_orthohelp(
        orthohelp_context,
        &["--cache", "--package", "orthohelp_fixture", "--locale", "en-US", "--locale", "fr-FR"],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    orthohelp_context.last_output.set(output);
    record_cache_state(orthohelp_context);
}

#[when("I rerun cargo-orthohelp with cache for the fixture")]
fn rerun_with_cache(orthohelp_context: &mut OrthoHelpContext) {
    // Ensure filesystem timestamp granularity distinguishes the cache file mtime.
    std::thread::sleep(Duration::from_secs(1));
    let output = run_orthohelp(
        orthohelp_context,
        &["--cache", "--package", "orthohelp_fixture", "--locale", "en-US", "--locale", "fr-FR"],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    orthohelp_context.last_output.set(output);
}

#[when("I run cargo-orthohelp with no-build for the fixture")]
fn run_with_no_build(orthohelp_context: &mut OrthoHelpContext) {
    let output = run_orthohelp(
        orthohelp_context,
        &["--no-build", "--package", "orthohelp_fixture", "--locale", "en-US"],
    );
    orthohelp_context.last_output.set(output);
}

#[then("the output contains localized IR JSON for {locale}")]
fn output_contains_locale(orthohelp_context: &mut OrthoHelpContext, locale: String) {
    orthohelp_context.last_output.with_ref(|output| {
        assert!(output.status.success(), "cargo-orthohelp should succeed");
    });

    let out_root = get_out_dir(orthohelp_context);
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
fn cached_ir_reused(orthohelp_context: &mut OrthoHelpContext) {
    let cache_path = orthohelp_context.cache_ir_path
        .with_ref(|p| p.clone())
        .expect("cached IR path should be recorded");
    let previous = orthohelp_context.cache_ir_mtime
        .with_ref(|m| *m)
        .expect("cached IR timestamp should be recorded");
    let cache_dir = cache_path.parent().expect("cached IR parent exists");
    let file_name = cache_path
        .file_name()
        .expect("cached IR filename should exist");
    let dir = Dir::open_ambient_dir(cache_dir, ambient_authority())
        .expect("open cache dir");
    let metadata = dir
        .metadata(file_name)
        .expect("cached IR metadata should be available");
    let current = metadata.modified().expect("cached IR mtime should exist");
    assert_eq!(previous, current, "cached IR should not be rewritten");
}

#[then("the cached IR deserializes into the schema")]
fn cached_ir_deserializes(orthohelp_context: &mut OrthoHelpContext) {
    let cache_path = orthohelp_context.cache_ir_path
        .with_ref(|p| p.clone())
        .expect("cached IR path should be recorded");
    let cache_dir = cache_path.parent().expect("cached IR parent exists");
    let file_name = cache_path
        .file_name()
        .expect("cached IR filename should exist");
    let dir = Dir::open_ambient_dir(cache_dir, ambient_authority())
        .expect("open cache dir");
    let mut file = dir.open(file_name).expect("cached IR should be readable");
    let mut json = String::new();
    file.read_to_string(&mut json)
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
fn command_fails_due_to_missing_cache(orthohelp_context: &mut OrthoHelpContext) {
    orthohelp_context.last_output.with_ref(|output| {
        assert!(!output.status.success(), "cargo-orthohelp should fail");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("cached IR missing"),
            "expected missing cache error, got: {stderr}"
        );
    });
}

fn run_orthohelp(ctx: &OrthoHelpContext, args: &[&str]) -> std::process::Output {
    let exe = cargo_orthohelp_exe();
    let workspace_root = get_workspace_root(ctx);
    let out_dir = get_out_dir(ctx);
    let mut command = Command::new(exe.as_str());
    command
        .current_dir(workspace_root.as_str())
        .arg("--out-dir")
        .arg(out_dir.as_str())
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

fn record_cache_state(ctx: &mut OrthoHelpContext) {
    let cache_path = find_cached_ir(ctx).expect("cached IR should exist");
    let cache_dir = cache_path.parent().expect("cached IR parent exists");
    let file_name = cache_path
        .file_name()
        .expect("cached IR filename should exist");
    let dir = Dir::open_ambient_dir(cache_dir, ambient_authority())
        .expect("open cache dir");
    let metadata = dir
        .metadata(file_name)
        .expect("cached IR metadata should be available");
    let modified = metadata.modified().expect("cached IR mtime should exist");
    ctx.cache_ir_path.set(cache_path);
    ctx.cache_ir_mtime.set(modified);
}

fn find_cached_ir(ctx: &OrthoHelpContext) -> Option<Utf8PathBuf> {
    let workspace_root = get_workspace_root(ctx);
    let cache_root = workspace_root
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
    let relative = Utf8PathBuf::from(file_name).join("ir.json");
    let dir = Dir::open_ambient_dir(cache_root.as_path(), ambient_authority()).ok()?;
    let metadata = dir.metadata(&relative).ok()?;
    let modified = metadata.modified().ok()?;
    let should_replace = newest
        .as_ref()
        .map_or(true, |(best_time, _)| modified > *best_time);
    if !should_replace {
        return None;
    }

    Some((modified, cache_root.join(relative)))
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

// --- Roff man page generation steps ---

#[when("I run cargo-orthohelp with format man for the fixture")]
fn run_with_format_man(orthohelp_context: &mut OrthoHelpContext) {
    let output = run_orthohelp(
        orthohelp_context,
        &["--format", "man", "--package", "orthohelp_fixture", "--locale", "en-US"],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed: {:?}", String::from_utf8_lossy(&output.stderr));
    orthohelp_context.last_output.set(output);
}

#[when("I run cargo-orthohelp with format man and section {section} for the fixture")]
fn run_with_format_man_section(orthohelp_context: &mut OrthoHelpContext, section: u8) {
    let section_str = section.to_string();
    let output = run_orthohelp(
        orthohelp_context,
        &[
            "--format", "man",
            "--man-section", &section_str,
            "--package", "orthohelp_fixture",
            "--locale", "en-US",
        ],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed: {:?}", String::from_utf8_lossy(&output.stderr));
    orthohelp_context.last_output.set(output);
}

#[when("I run cargo-orthohelp with format all for the fixture")]
fn run_with_format_all(orthohelp_context: &mut OrthoHelpContext) {
    let output = run_orthohelp(
        orthohelp_context,
        &["--format", "all", "--package", "orthohelp_fixture", "--locale", "en-US"],
    );
    assert!(output.status.success(), "cargo-orthohelp should succeed: {:?}", String::from_utf8_lossy(&output.stderr));
    orthohelp_context.last_output.set(output);
}

#[then("the output contains a man page for {name}")]
fn output_contains_man_page(orthohelp_context: &mut OrthoHelpContext, name: String) {
    let out_root = get_out_dir(orthohelp_context);
    let man_path = out_root.join(format!("man/man1/{name}.1"));
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())
        .expect("open output dir");

    let mut file = dir
        .open(&Utf8PathBuf::from(format!("man/man1/{name}.1")))
        .unwrap_or_else(|e| panic!("man page should exist at {man_path}: {e}"));

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("read man page content");

    assert!(content.contains(".TH"), "man page should contain .TH header");
}

#[then("the output contains a man page at section {section} for {name}")]
fn output_contains_man_page_section(orthohelp_context: &mut OrthoHelpContext, section: u8, name: String) {
    let out_root = get_out_dir(orthohelp_context);
    let man_path = out_root.join(format!("man/man{section}/{name}.{section}"));
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())
        .expect("open output dir");

    let mut file = dir
        .open(&Utf8PathBuf::from(format!("man/man{section}/{name}.{section}")))
        .unwrap_or_else(|e| panic!("man page should exist at {man_path}: {e}"));

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("read man page content");

    assert!(
        content.contains(&format!(".TH \"{}\" \"{section}\"", name.to_uppercase())),
        "man page should have correct .TH header for section {section}"
    );
}

#[then("the man page contains section {section_name}")]
fn man_page_contains_section(orthohelp_context: &mut OrthoHelpContext, section_name: String) {
    let out_root = get_out_dir(orthohelp_context);
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())
        .expect("open output dir");

    let mut file = dir
        .open(&Utf8PathBuf::from("man/man1/orthohelp_fixture.1"))
        .expect("man page should exist");

    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("read man page content");

    assert!(
        content.contains(&format!(".SH {section_name}")),
        "man page should contain .SH {section_name} section"
    );
}
