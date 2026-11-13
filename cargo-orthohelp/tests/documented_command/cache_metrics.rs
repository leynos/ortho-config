//! Bridge-cache metric contracts for the documented `cargo orthohelp` runs.
//!
//! These tests drive the real binary with `--cache` and read the exported
//! counters from its standard error, which is the only evidence that the
//! recorder `main` installs claims the `metrics` global slot. The helpers that
//! clear and match cache entries live here because no other lane needs them.

use super::{DOCUMENTED_RUN_MUTEX, build_state, run_documented_command};
use anyhow::{Context, Result, ensure};
use cap_std::{
    ambient_authority,
    fs::{Dir, DirEntry},
};
use serde_json::Value;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::PoisonError;
use std::time::Duration;

/// One bridge-cache operation, as its bounded metric label names it.
#[derive(Clone, Copy, Debug)]
enum CacheOperation {
    /// Reading the cached IR before a build.
    Lookup,
    /// Writing generated IR back to the cache.
    Publish,
}

impl CacheOperation {
    /// The label this operation is exported under.
    const fn label(self) -> &'static str {
        match self {
            Self::Lookup => "lookup",
            Self::Publish => "publish",
        }
    }
}

/// One bridge-cache metric event, as its bounded labels name it.
#[derive(Clone, Copy, Debug)]
struct CacheEvent {
    /// The cache operation the counter describes.
    operation: CacheOperation,
    /// The outcome that counter recorded.
    outcome: &'static str,
}

impl CacheEvent {
    /// Name one exported cache event.
    const fn new(operation: CacheOperation, outcome: &'static str) -> Self {
        Self { operation, outcome }
    }
}

/// The exported bridge-cache event lines within one diagnostic stream.
fn cache_event_lines(stderr: &str) -> impl Iterator<Item = &str> {
    stderr
        .lines()
        .filter(|line| line.contains("cargo_orthohelp.bridge_cache.events"))
}

/// Count exported bridge-cache metric events matching one operation and
/// outcome.
fn cache_event_count(stderr: &str, event: CacheEvent) -> usize {
    let operation_label = format!("operation={}", event.operation.label());
    let outcome_label = format!("outcome={}", event.outcome);
    cache_event_lines(stderr)
        .filter(|line| line.contains(&operation_label))
        .filter(|line| line.contains(&outcome_label))
        .count()
}

/// Count exported bridge-cache events for one operation, whatever its outcome.
///
/// An outcome-blind count is what lets a test assert an operation did not
/// happen at all: a per-outcome count still passes when the operation reports
/// an outcome the test never named.
fn cache_operation_count(stderr: &str, operation: CacheOperation) -> usize {
    let operation_label = format!("operation={}", operation.label());
    cache_event_lines(stderr)
        .filter(|line| line.contains(&operation_label))
        .count()
}

/// Delete the bridge-cache entries generated for one application.
///
/// The cache directory is named from a hash of the package files, the schema,
/// and the tool version, so a test cannot name the entry it wants. Every entry
/// records the application name it was generated for, which identifies the
/// fixture exactly. Other scenarios share this target and match a different
/// name, so their entries must survive; a missing cache root simply means the
/// next run is cold for every package.
fn clear_cached_ir(target: &Path, app_name: &str) -> Result<()> {
    let Some(cache) = open_cache_root(target)? else {
        return Ok(());
    };
    for candidate in cache.entries()? {
        remove_fixture_entry(&cache, &candidate?, app_name)?;
    }
    Ok(())
}

/// Open the cache root, treating an absent root as an already-cold cache.
///
/// Only absence means "nothing cached". Any other open failure is reported:
/// folding it into an empty cache would let an unreadable target directory look
/// like a cache this test had already cleared, and the cold-run assertions
/// would then be checking a state nothing established.
fn open_cache_root(target: &Path) -> Result<Option<Dir>> {
    let cache_root = target.join("orthohelp");
    let opened = Dir::open_ambient_dir(&cache_root, ambient_authority());
    if opened.as_ref().err().map(std::io::Error::kind) == Some(std::io::ErrorKind::NotFound) {
        return Ok(None);
    }
    let cache = opened.with_context(|| format!("open {}", cache_root.display()))?;
    Ok(Some(cache))
}

/// Remove one cache entry when it holds IR generated for `app_name`.
///
/// Entries this repository did not write — another application's, a partial
/// publish, unreadable IR — are not ours to delete, so they stay.
fn remove_fixture_entry(cache: &Dir, entry: &DirEntry, app_name: &str) -> Result<()> {
    let name = entry.file_name();
    if entry.file_type()?.is_dir() && is_fixture_ir(cache, &name, app_name) {
        cache
            .remove_dir_all(&name)
            .with_context(|| format!("remove cached IR entry {}", name.to_string_lossy()))?;
    }
    Ok(())
}

/// Report whether a cache entry records `app_name` as the application it was
/// generated for.
///
/// An entry that cannot be read or parsed as JSON is not evidence about the
/// fixture, so it counts as another application's and is left alone.
fn is_fixture_ir(cache: &Dir, name: &OsStr, app_name: &str) -> bool {
    let Ok(directory) = cache.open_dir(name) else {
        return false;
    };
    let Ok(ir_text) = directory.read_to_string("ir.json") else {
        return false;
    };
    let Ok(ir) = serde_json::from_str::<Value>(&ir_text) else {
        return false;
    };
    ir.get("app_name").and_then(Value::as_str) == Some(app_name)
}

/// The recorder `main` installs exports bridge-cache counters to stderr.
///
/// The `metrics` facade discards updates until a recorder claims its global
/// slot, so this test drives the real binary with `--cache` twice and reads the
/// exported events from its standard error. Removing the install hook from
/// `main`, changing the `tracing` target, or dropping the bounded labels fails
/// it, which the in-process unit test cannot detect because that test installs
/// the recorder itself.
///
/// The test removes the fixture's cached IR before the first run, so that run
/// is certain to miss and the second to hit. Clearing the entry outright,
/// rather than tolerating a cache another test warmed, is what makes the
/// publication path part of the contract instead of an occasional observation.
///
/// The clear and both runs share one critical section. Another test in this
/// binary runs the same fixture without `--cache`, and a run that builds the
/// bridge publishes its IR whether or not it was asked to read the cache, so a
/// clear outside the lock could be undone before the first probe.
#[test]
fn cache_metrics_reach_stderr_through_the_cli_recorder() -> Result<()> {
    const ENVIRONMENT: [(&str, &str); 1] = [("RUST_LOG", "cargo_orthohelp::metrics=debug")];
    const ARGS: [&str; 3] = ["--package", "hello_world", "--cache"];

    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("workspace repository")?;
    let _run_guard = DOCUMENTED_RUN_MUTEX
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let state = build_state::BuildState::discover(repository, Duration::from_secs(30))?;
    clear_cached_ir(&state.target, "hello_world")?;

    let first = run_documented_command(&ARGS, &ENVIRONMENT)?;
    first.ensure_success("metrics-enabled cold run")?;
    let first_stderr = String::from_utf8_lossy(&first.output.stderr).into_owned();
    ensure!(
        cache_event_count(
            &first_stderr,
            CacheEvent::new(CacheOperation::Lookup, "miss")
        ) == 1,
        "the cold run should export exactly one lookup miss:\n{first_stderr}"
    );
    ensure!(
        cache_event_count(
            &first_stderr,
            CacheEvent::new(CacheOperation::Lookup, "hit")
        ) == 0,
        "the cold run cannot observe a hit:\n{first_stderr}"
    );
    ensure!(
        cache_event_count(
            &first_stderr,
            CacheEvent::new(CacheOperation::Publish, "updated")
        ) == 1,
        "a cache miss should publish the generated IR exactly once:\n{first_stderr}"
    );

    let second = run_documented_command(&ARGS, &ENVIRONMENT)?;
    second.ensure_success("metrics-enabled warm run")?;
    let second_stderr = String::from_utf8_lossy(&second.output.stderr).into_owned();
    ensure!(
        cache_event_count(
            &second_stderr,
            CacheEvent::new(CacheOperation::Lookup, "hit")
        ) == 1,
        "the repeat run should hit the IR published by the first:\n{second_stderr}"
    );
    ensure!(
        cache_operation_count(&second_stderr, CacheOperation::Publish) == 0,
        "a cache hit must not publish the IR under any outcome:\n{second_stderr}"
    );
    Ok(())
}
