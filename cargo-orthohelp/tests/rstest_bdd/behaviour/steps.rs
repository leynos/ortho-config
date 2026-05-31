//! Shared state and re-exports for `cargo-orthohelp` behavioural steps.

use std::sync::{Mutex, MutexGuard};

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
    scenario_lock: Slot<MutexGuard<'static, ()>>,
    pub workspace_root: Slot<Utf8PathBuf>,
    pub out_dir: Slot<TempDir>,
    pub last_output: Slot<std::process::Output>,
    pub cache_ir_path: Slot<Utf8PathBuf>,
    pub cache_ir_content: Slot<String>,
}

impl Default for OrthoHelpContext {
    fn default() -> Self {
        let scenario_lock = match SCENARIO_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let ctx = Self {
            scenario_lock: Slot::new(),
            workspace_root: Slot::new(),
            out_dir: Slot::new(),
            last_output: Slot::new(),
            cache_ir_path: Slot::new(),
            cache_ir_content: Slot::new(),
        };
        ctx.scenario_lock.set(scenario_lock);
        ctx
    }
}

static SCENARIO_LOCK: Mutex<()> = Mutex::new(());

/// Provides a clean context for orthohelp scenarios.
#[fixture]
pub fn orthohelp_context() -> OrthoHelpContext {
    let workspace_root = match fixtures::workspace_root() {
        Ok(root) => root,
        Err(err) => panic!("workspace root should exist: {err}"),
    };
    let ctx = OrthoHelpContext::default();
    ctx.workspace_root.set(workspace_root);
    let out_dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(err) => panic!("temporary output directory should be created: {err}"),
    };
    ctx.out_dir.set(out_dir);
    ctx
}
