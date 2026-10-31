//! End-to-end proof that declared renderer metadata reaches a real consumer.
//!
//! The behavioural scenarios assert the same contract against the same fixture,
//! but they run inside the test process's own harness. This file closes the last
//! gap: it spawns the shipped `cargo-orthohelp` binary as a subprocess with
//! stdin closed and a wall-clock deadline, then parses the two artefacts an
//! agent actually reads — compact stdout and `agent-context.json` — to confirm
//! they agree.
//!
//! Both formats are exercised because they are written by separate calls
//! (`write_agent_context_stdout` and `write_agent_context`), so a change that
//! serialized one and not the other would still pass a single-artefact test.
//!
//! See ADR-0016 for the renderer vocabulary. `nested-fixture greet` declares
//! both renderers; `nested-fixture version` deliberately declares none and is
//! the witness for the legacy defaults.

mod fixtures;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use serde_json::Value;
use std::error::Error;
use std::io::Read;
use std::process::Command;
use std::sync::{LazyLock, Mutex, PoisonError};
use tempfile::TempDir;

// The shared harness serves the documentation-example suites as well, so it
// exposes more than this file calls. Silence only the dead-code and unused-import
// noise that comes with that sharing; the modules' own suites still exercise the
// remaining surface, and `-D warnings` would otherwise reject this target.
#[allow(dead_code, unused_imports)]
#[path = "../../ortho_config/tests/documentation_examples/cargo_environment.rs"]
mod cargo_environment;
#[allow(dead_code, unused_imports)]
#[path = "../../ortho_config/tests/documentation_examples/process_runner_support.rs"]
mod process_runner;

type TestError = Box<dyn Error + Send + Sync>;

const FIXTURE_PACKAGE: &str = "orthohelp_fixture";
const FIXTURE_ROOT_TYPE: &str = "orthohelp_fixture::NestedFixtureConfig";

/// Package-rooted leaf of the command that declares both renderers.
const GREET_COMMAND: &str = "greet";
/// Leaf of the command that declares no renderer at all.
const VERSION_COMMAND: &str = "version";

// The bridge directory is content-addressed and rewritten before each build, so
// two scenarios must never overlap. The BDD suite takes the same precaution.
static BRIDGE_BUILD_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

/// Both generated artefacts must carry the same declared renderer block.
///
/// Comparing the two documents to each other and to an expected value means the
/// test fails whether the metadata is dropped, altered on one path only, or
/// invented for a command that never declared anything.
#[test]
fn declared_renderer_metadata_survives_to_both_artefacts() -> Result<(), TestError> {
    let (_temp, out_dir) = temp_out_dir()?;
    let run = run_orthohelp(&out_dir)?;

    let stdout: Value = serde_json::from_slice(&run.stdout).map_err(|error| {
        format!(
            "stdout should be one compact JSON document, got {:?}: {error}",
            String::from_utf8_lossy(&run.stdout)
        )
    })?;
    let artefact = read_artefact(&out_dir)?;

    let declared = expected_greet_renderer();
    for (origin, document) in [("stdout", &stdout), ("agent-context.json", &artefact)] {
        let greet = command_by_leaf(document, GREET_COMMAND)
            .map_err(|error| format!("{origin}: {error}"))?;
        let renderer = greet
            .get("renderer")
            .ok_or_else(|| format!("{origin}: greet command carries no renderer block"))?;
        if *renderer != declared {
            return Err(
                format!("{origin}: greet renderer should be {declared}, got {renderer}").into(),
            );
        }
    }

    if stdout != artefact {
        return Err("stdout and agent-context.json should hold the same document".into());
    }
    Ok(())
}

/// A command that declares nothing reports the legacy defaults and nothing more.
///
/// The two support claims are opposite, so an implementation that defaulted
/// machine support to `true` — or copied the neighbour's declaration — fails
/// here even though `greet` would still pass.
#[test]
fn undeclared_command_keeps_legacy_defaults_end_to_end() -> Result<(), TestError> {
    let (_temp, out_dir) = temp_out_dir()?;
    let run = run_orthohelp(&out_dir)?;
    let stdout: Value = serde_json::from_slice(&run.stdout)?;

    let version = command_by_leaf(&stdout, VERSION_COMMAND)?;
    let renderer = version
        .get("renderer")
        .ok_or("an undeclared command should still report the default contract")?;

    let read_supported = |role: &str| -> Result<bool, TestError> {
        renderer
            .get(role)
            .and_then(|record| record.get("supported"))
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("{role} renderer supported should be a boolean").into())
    };
    assert_eq!(
        read_supported("human")?,
        true,
        "human support legacy default"
    );
    assert_eq!(
        read_supported("machine")?,
        false,
        "machine support legacy default"
    );

    // Absence, not a reassuring false, is how an undeclared policy is reported.
    for role in ["human", "machine"] {
        let record = &renderer[role];
        for key in [
            "tty", "colour", "width", "stdout", "stderr", "progress", "pager",
        ] {
            if record.get(key).is_some() {
                return Err(format!(
                    "{role} renderer should make no {key} claim, got {}",
                    record[key]
                )
                .into());
            }
        }
    }
    Ok(())
}

/// The renderer block `nested-fixture greet` declares, as it appears on the wire.
///
/// Written as a literal rather than derived from the fixture so the assertion
/// pins the wire spelling a consumer depends on.
fn expected_greet_renderer() -> Value {
    serde_json::json!({
        "human": {
            "supported": true,
            "tty": "adapts",
            "colour": "auto",
            "width": {"kind": "terminal"},
            "plain": "available",
            "stdout": "text",
            "stderr": "text",
            "fields": [
                {"id": "message", "localization": "localized"},
                {"id": "error_code", "localization": "non_localized"}
            ]
        },
        "machine": {
            "supported": true,
            "tty": "independent",
            "colour": "never",
            "progress": "never",
            "pager": "never",
            "width": {"kind": "unbounded"},
            "plain": "always",
            "stdout": "structured",
            "stderr": "structured",
            "fields": [
                {"id": "error_code", "localization": "non_localized"}
            ]
        }
    })
}

/// Runs the fixture through `--format agent-context` with closed stdin.
///
/// The shared runner closes stdin and enforces a deadline, so a generator that
/// waited for input or hung would be reported as such rather than stalling the
/// suite until the harness gives up.
fn run_orthohelp(out_dir: &Utf8PathBuf) -> Result<std::process::Output, TestError> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    let workspace_root = fixtures::workspace_root()?;
    let _bridge_build_guard = BRIDGE_BUILD_MUTEX
        .lock()
        .unwrap_or_else(PoisonError::into_inner);

    let mut command = Command::new(exe.as_str());
    cargo_environment::CargoEnvironment::build().apply(&mut command);
    command
        .current_dir(workspace_root.as_str())
        .arg("orthohelp")
        .arg("--out-dir")
        .arg(out_dir.as_str())
        .arg("--package")
        .arg(FIXTURE_PACKAGE)
        .arg("--root-type")
        .arg(FIXTURE_ROOT_TYPE)
        .arg("--locale")
        .arg("en-US")
        .arg("--format")
        .arg("agent-context");

    let output = process_runner::run_with_policy(
        &mut command,
        process_runner::Operation("run cargo-orthohelp agent-context"),
        process_runner::Policy::build(),
    )?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(format!(
            "cargo-orthohelp should succeed, stderr was {:?}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into())
    }
}

/// Reads and parses the generated `agent-context.json` artefact.
fn read_artefact(out_dir: &Utf8PathBuf) -> Result<Value, TestError> {
    let dir = Dir::open_ambient_dir(out_dir, ambient_authority())?;
    let mut file = dir.open(Utf8Path::new("agent-context.json"))?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(serde_json::from_str(&content)?)
}

/// Finds the single command whose `path` ends in `leaf`.
///
/// Requiring uniqueness means a renamed or duplicated command fails here rather
/// than silently asserting whichever match came first.
fn command_by_leaf<'a>(document: &'a Value, leaf: &str) -> Result<&'a Value, TestError> {
    let matches = document
        .get("commands")
        .and_then(Value::as_array)
        .ok_or("commands should be an array")?
        .iter()
        .filter(|command| {
            command
                .get("path")
                .and_then(Value::as_array)
                .and_then(|path| path.last())
                .and_then(Value::as_str)
                == Some(leaf)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [command] => Ok(command),
        other => Err(format!("expected one command named {leaf:?}, found {}", other.len()).into()),
    }
}

/// Creates an output directory and keeps its temporary owner alive.
fn temp_out_dir() -> Result<(TempDir, Utf8PathBuf), TestError> {
    let temp_dir = tempfile::tempdir()?;
    let out_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    Ok((temp_dir, out_dir))
}
