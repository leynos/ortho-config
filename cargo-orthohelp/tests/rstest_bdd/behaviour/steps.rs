//! Shared state and re-exports for `cargo-orthohelp` behavioural steps.

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use tempfile::TempDir;

use crate::fixtures;

pub use super::steps_cmd::{get_out_dir, run_orthohelp};

/// Error type for step definition failures.
pub type StepError = Box<dyn std::error::Error + Send + Sync>;

/// Result type for step definition operations.
pub type StepResult<T> = Result<T, StepError>;

/// Scenario state for cargo-orthohelp scenarios.
#[derive(Debug, ScenarioState)]
pub struct OrthoHelpContext {
    pub workspace_root: Slot<Utf8PathBuf>,
    pub out_dir: Slot<TempDir>,
    pub last_output: Slot<std::process::Output>,
    pub cache_ir_path: Slot<Utf8PathBuf>,
    pub cache_ir_content: Slot<String>,
}

impl Default for OrthoHelpContext {
    fn default() -> Self {
        // Serialization of BDD scenarios is enforced at the nextest level via
        // `max-threads = 1` in `.config/nextest.toml`. A process-local Mutex
        // cannot guard against concurrent access from separate nextest
        // processes, so none is used here.
        Self {
            workspace_root: Slot::new(),
            out_dir: Slot::new(),
            last_output: Slot::new(),
            cache_ir_path: Slot::new(),
            cache_ir_content: Slot::new(),
        }
    }
}

/// Provides a clean context for orthohelp scenarios.
#[fixture]
pub fn orthohelp_context() -> OrthoHelpContext {
    let workspace_root = match fixtures::workspace_root() {
        Ok(root) => root,
        // `rstest-bdd` requires this fixture to return the exact scenario
        // state type, so setup failures cannot be propagated as StepResult.
        Err(err) => panic!("workspace root should exist: {err}"),
    };
    let ctx = OrthoHelpContext::default();
    ctx.workspace_root.set(workspace_root);
    let out_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        // `rstest-bdd` requires this fixture to return the exact scenario
        // state type, so setup failures cannot be propagated as StepResult.
        Err(err) => panic!("temporary output directory should be created: {err}"),
    };
    ctx.out_dir.set(out_dir);
    ctx
}
