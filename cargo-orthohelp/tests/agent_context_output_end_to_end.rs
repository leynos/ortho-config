//! End-to-end proof that declared structured-output metadata reaches a consumer.
//!
//! The unit and property suites prove the bridge maps declarations correctly;
//! this file closes the last gap by spawning the shipped `cargo-orthohelp`
//! binary as a subprocess with stdin closed and a wall-clock deadline, then
//! reading the two artefacts an agent actually consumes — compact stdout and
//! `agent-context.json`.
//!
//! Both writers are exercised because they are separate calls, so a change that
//! serialized one and not the other would still pass a single-artefact test.
//!
//! Three fixture commands divide the contract between them: `greet` declares a
//! capability, `audit` explicitly refuses one, and `version` declares nothing.
//! The refusal and the silence must stay distinguishable on the wire, which is
//! why the last scenario also runs a second locale: non-localized metadata must
//! not vary with the human language.

mod fixtures;

#[path = "fixtures/wire_assertions.rs"]
mod wire_assertions;

// Declared here rather than inside `fixtures` so targets that never read
// structured-output metadata do not carry these helpers as dead code.
#[path = "fixtures/output_assertions.rs"]
mod output_assertions;

// The shared harness serves the documentation-example suites as well, so it
// exposes more than this file calls. Expect only the dead-code noise that comes
// with that sharing; the modules' own suites still exercise the remaining
// surface, and `-D warnings` would otherwise reject this target.
#[expect(
    dead_code,
    reason = "shared harness exposes more than this target calls"
)]
#[path = "../../ortho_config/tests/documentation_examples/cargo_environment.rs"]
mod cargo_environment;
#[expect(
    dead_code,
    unused_imports,
    reason = "shared harness exposes more than this target calls"
)]
#[path = "../../ortho_config/tests/documentation_examples/process_runner_support.rs"]
mod process_runner;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use serde_json::Value;
use std::error::Error;
use std::io::Read;
use std::process::Command;
use std::sync::{LazyLock, Mutex, PoisonError};
use tempfile::TempDir;

type TestError = Box<dyn Error + Send + Sync>;

const FIXTURE_PACKAGE: &str = "orthohelp_fixture";
const FIXTURE_ROOT_TYPE: &str = "orthohelp_fixture::NestedFixtureConfig";
const AGENT_CONTEXT_FORMAT: &str = "agent-context";

// The bridge directory is content-addressed and rewritten before each build, so
// two scenarios must never overlap. The BDD suite takes the same precaution.
static BRIDGE_BUILD_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

/// Both generated artefacts must carry the same declared output block.
#[test]
fn declared_output_metadata_survives_to_both_artefacts() -> Result<(), TestError> {
    let (_temp, out_dir) = temp_out_dir()?;
    let run = run_orthohelp(&out_dir, "en-US")?;

    let stdout: Value = serde_json::from_slice(&run.stdout).map_err(|error| {
        format!(
            "stdout should be one compact JSON document, got {:?}: {error}",
            String::from_utf8_lossy(&run.stdout)
        )
    })?;
    let artefact = read_artefact(&out_dir)?;

    for (origin, document) in [("stdout", &stdout), ("agent-context.json", &artefact)] {
        output_assertions::assert_greet_output(document, origin)?;
    }

    if stdout != artefact {
        return Err("stdout and agent-context.json should hold the same document".into());
    }
    Ok(())
}

/// A refusal stays declared while silence stays silent.
#[test]
fn refusal_and_silence_stay_distinguishable() -> Result<(), TestError> {
    let (_temp, out_dir) = temp_out_dir()?;
    let run = run_orthohelp(&out_dir, "en-US")?;
    let stdout: Value = serde_json::from_slice(&run.stdout)?;

    output_assertions::assert_audit_refusal(&stdout, "stdout")?;
    output_assertions::assert_version_silence(&stdout, "stdout")
}

/// Output metadata does not vary with the locale an agent asks for.
///
/// The declarations are not localized text, so a run in another language must
/// produce the same block. Comparing two independent runs also proves the
/// metadata survives localization rather than only the unlocalized path.
#[test]
fn output_metadata_is_locale_independent() -> Result<(), TestError> {
    let (_first_temp, first_dir) = temp_out_dir()?;
    let first = run_orthohelp(&first_dir, "en-US")?;
    let first: Value = serde_json::from_slice(&first.stdout)?;

    let (_second_temp, second_dir) = temp_out_dir()?;
    let second = run_orthohelp(&second_dir, "fr-FR")?;
    let second: Value = serde_json::from_slice(&second.stdout)?;

    output_assertions::assert_greet_output(&first, "en-US stdout")?;
    output_assertions::assert_greet_output(&second, "fr-FR stdout")?;
    Ok(())
}

/// Runs the fixture through the agent-context format with closed stdin.
///
/// The shared runner closes stdin and enforces a deadline, so a generator that
/// waited for input or hung would be reported as such rather than stalling the
/// suite until the harness gives up.
fn run_orthohelp(out_dir: &Utf8PathBuf, locale: &str) -> Result<std::process::Output, TestError> {
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
        .arg(locale)
        .arg("--format")
        .arg(AGENT_CONTEXT_FORMAT);

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

/// Creates an output directory and keeps its temporary owner alive.
fn temp_out_dir() -> Result<(TempDir, Utf8PathBuf), TestError> {
    let temp_dir = tempfile::tempdir()?;
    let out_dir = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf())
        .map_err(|path| format!("temporary path is not UTF-8: {}", path.display()))?;
    Ok((temp_dir, out_dir))
}
