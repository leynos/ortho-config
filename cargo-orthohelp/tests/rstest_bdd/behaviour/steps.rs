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

/// Error type for step definition failures.
pub type StepError = Box<dyn std::error::Error + Send + Sync>;

/// Result type for step definition operations.
pub type StepResult<T> = Result<T, StepError>;

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
pub fn orthohelp_context() -> StepResult<OrthoHelpContext> {
    let manifest_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .ok_or("workspace root should exist")?
        .to_path_buf();
    let ctx = OrthoHelpContext::default();
    ctx.workspace_root.set(workspace_root);
    ctx.out_dir.set(tempfile::tempdir()?);
    Ok(ctx)
}

/// Gets the output directory path from the context.
pub fn get_out_dir(ctx: &OrthoHelpContext) -> StepResult<Utf8PathBuf> {
    ctx.out_dir
        .with_ref(|dir| {
            Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
                .map_err(|p| format!("non-UTF-8 path: {}", p.display()))
        })
        .ok_or_else(|| "out_dir should be set".to_owned())?
}

/// Gets the workspace root path from the context.
fn get_workspace_root(ctx: &OrthoHelpContext) -> StepResult<Utf8PathBuf> {
    ctx.workspace_root
        .with_ref(|root| root.clone())
        .ok_or_else(|| "workspace_root should be set".into())
}

#[given("a temporary output directory")]
fn temp_output_dir(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let path = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&path, ambient_authority())?;
    let entries = dir.read_dir(".")?;
    assert_eq!(entries.count(), 0, "output dir should start empty");
    Ok(())
}

#[given("the orthohelp cache is empty")]
fn cache_is_empty(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let workspace_root = get_workspace_root(orthohelp_context)?;
    let root_dir = Dir::open_ambient_dir(workspace_root.as_path(), ambient_authority())?;
    if let Err(err) = root_dir.remove_dir_all("target/orthohelp") {
        if !is_not_found_kind(&err) {
            return Err(format!("remove orthohelp cache failed: {err}").into());
        }
    }
    orthohelp_context.cache_ir_path.clear();
    orthohelp_context.cache_ir_mtime.clear();
    Ok(())
}

fn is_not_found_kind(err: &std::io::Error) -> bool {
    matches!(err.kind(), std::io::ErrorKind::NotFound)
}

#[when("I run cargo-orthohelp with cache for the fixture")]
fn run_with_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output = run_orthohelp(
        orthohelp_context,
        &["--cache", "--package", "orthohelp_fixture", "--locale", "en-US", "--locale", "fr-FR"],
    )?;
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    orthohelp_context.last_output.set(output);
    record_cache_state(orthohelp_context)?;
    Ok(())
}

#[when("I rerun cargo-orthohelp with cache for the fixture")]
fn rerun_with_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    // Ensure filesystem timestamp granularity distinguishes the cache file mtime.
    std::thread::sleep(Duration::from_secs(1));
    let output = run_orthohelp(
        orthohelp_context,
        &["--cache", "--package", "orthohelp_fixture", "--locale", "en-US", "--locale", "fr-FR"],
    )?;
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    orthohelp_context.last_output.set(output);
    Ok(())
}

#[when("I run cargo-orthohelp with no-build for the fixture")]
fn run_with_no_build(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output = run_orthohelp(
        orthohelp_context,
        &["--no-build", "--package", "orthohelp_fixture", "--locale", "en-US"],
    )?;
    orthohelp_context.last_output.set(output);
    Ok(())
}

#[then("the output contains localised IR JSON for {locale}")]
fn output_contains_locale(orthohelp_context: &mut OrthoHelpContext, locale: String) -> StepResult<()> {
    orthohelp_context.last_output.with_ref(|output| {
        assert!(output.status.success(), "cargo-orthohelp should succeed");
    });

    let out_root = get_out_dir(orthohelp_context)?;
    let dir = Dir::open_ambient_dir(&out_root, ambient_authority())?;
    let mut file = dir.open(&Utf8PathBuf::from(format!("ir/{locale}.json")))?;

    let mut buffer = String::new();
    file.read_to_string(&mut buffer)?;

    let json: Value = serde_json::from_str(&buffer)?;
    let ir_version = json
        .get("ir_version")
        .and_then(Value::as_str)
        .ok_or("ir_version field missing")?;
    assert_eq!(
        ir_version,
        ortho_config::docs::ORTHO_DOCS_IR_VERSION,
        "IR version should match schema"
    );
    let json_locale = json
        .get("locale")
        .and_then(Value::as_str)
        .ok_or("locale field missing")?;
    assert_eq!(json_locale, locale);
    let about = json
        .get("about")
        .and_then(Value::as_str)
        .ok_or("about field missing")?;
    assert_eq!(about, expected_about(&locale));

    let help = json
        .get("fields")
        .and_then(Value::as_array)
        .and_then(|fields| fields.first())
        .and_then(|field| field.get("help"))
        .and_then(Value::as_str)
        .ok_or("field help missing")?;
    assert_eq!(help, expected_help(&locale));
    Ok(())
}

#[then("the cached IR is reused")]
fn cached_ir_reused(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let cache_path = orthohelp_context.cache_ir_path
        .with_ref(|p| p.clone())
        .ok_or("cached IR path should be recorded")?;
    let previous = orthohelp_context.cache_ir_mtime
        .with_ref(|m| *m)
        .ok_or("cached IR timestamp should be recorded")?;
    let cache_dir = cache_path.parent().ok_or("cached IR parent missing")?;
    let file_name = cache_path
        .file_name()
        .ok_or("cached IR filename missing")?;
    let dir = Dir::open_ambient_dir(cache_dir, ambient_authority())?;
    let metadata = dir.metadata(file_name)?;
    let current = metadata.modified()?;
    assert_eq!(previous, current, "cached IR should not be rewritten");
    Ok(())
}

#[then("the cached IR deserialises into the schema")]
fn cached_ir_deserialises(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let cache_path = orthohelp_context.cache_ir_path
        .with_ref(|p| p.clone())
        .ok_or("cached IR path should be recorded")?;
    let cache_dir = cache_path.parent().ok_or("cached IR parent missing")?;
    let file_name = cache_path
        .file_name()
        .ok_or("cached IR filename missing")?;
    let dir = Dir::open_ambient_dir(cache_dir, ambient_authority())?;
    let mut file = dir.open(file_name)?;
    let mut json = String::new();
    file.read_to_string(&mut json)?;
    let metadata: ortho_config::docs::DocMetadata = serde_json::from_str(&json)?;
    assert_eq!(
        metadata.ir_version,
        ortho_config::docs::ORTHO_DOCS_IR_VERSION,
        "cached IR should match the current schema version"
    );
    Ok(())
}

#[then("the command fails due to missing cache")]
fn command_fails_due_to_missing_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    orthohelp_context
        .last_output
        .with_ref(|output| {
            assert!(!output.status.success(), "cargo-orthohelp should fail");
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains("cached IR missing"),
                "expected missing cache error, got: {stderr}"
            );
        })
        .ok_or("last_output should be set")?;
    Ok(())
}

/// Runs cargo-orthohelp with the given arguments.
pub fn run_orthohelp(ctx: &OrthoHelpContext, args: &[&str]) -> StepResult<std::process::Output> {
    let exe = cargo_orthohelp_exe()?;
    let workspace_root = get_workspace_root(ctx)?;
    let out_dir = get_out_dir(ctx)?;
    let mut command = Command::new(exe.as_str());
    command
        .current_dir(workspace_root.as_str())
        .arg("--out-dir")
        .arg(out_dir.as_str())
        .args(args);
    Ok(command.output()?)
}

fn cargo_orthohelp_exe() -> StepResult<Utf8PathBuf> {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_cargo-orthohelp") {
        return Ok(Utf8PathBuf::from(path));
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_cargo_orthohelp") {
        return Ok(Utf8PathBuf::from(path));
    }
    Err("cargo-orthohelp binary path not found in environment".into())
}

fn record_cache_state(ctx: &mut OrthoHelpContext) -> StepResult<()> {
    let cache_path = find_cached_ir(ctx)?.ok_or("cached IR should exist")?;
    let cache_dir = cache_path.parent().ok_or("cached IR parent missing")?;
    let file_name = cache_path
        .file_name()
        .ok_or("cached IR filename missing")?;
    let dir = Dir::open_ambient_dir(cache_dir, ambient_authority())?;
    let metadata = dir.metadata(file_name)?;
    let modified = metadata.modified()?;
    ctx.cache_ir_path.set(cache_path);
    ctx.cache_ir_mtime.set(modified);
    Ok(())
}

fn find_cached_ir(ctx: &OrthoHelpContext) -> StepResult<Option<Utf8PathBuf>> {
    let workspace_root = get_workspace_root(ctx)?;
    let cache_root = workspace_root
        .join("target")
        .join("orthohelp");
    let dir = match Dir::open_ambient_dir(&cache_root, ambient_authority()) {
        Ok(d) => d,
        Err(e) if is_not_found_kind(&e) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut newest: Option<(SystemTime, Utf8PathBuf)> = None;
    for entry in dir.read_dir(".")? {
        if let Some(candidate) = check_cache_entry(&cache_root, entry, &newest) {
            newest = Some(candidate);
        }
    }
    Ok(newest.map(|(_, path)| path))
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
