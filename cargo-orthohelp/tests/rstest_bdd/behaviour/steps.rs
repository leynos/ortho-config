//! Shared scenario state and re-exports for `cargo-orthohelp` behavioural steps.
//!
//! Defines [`OrthoHelpContext`], the rstest-bdd [`ScenarioState`] struct that
//! accumulates per-scenario data (workspace root, temporary output directory,
//! last command output, and cached IR path/content). Also exposes the
//! [`orthohelp_context`] rstest fixture that initialises this state, and
//! target-directory helpers ([`resolve_target_dir`], [`scenario_target_dir`])
//! shared by [`super::steps_cmd`] and [`super::steps_cache`].
//!
//! All other step modules import [`OrthoHelpContext`] and [`StepResult`] from
//! here; none of them import from each other directly.

use camino::{Utf8Path, Utf8PathBuf};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::ScenarioState;
use tempfile::TempDir;

use crate::fixtures;

pub use super::steps_cmd::run_orthohelp;

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

/// Gets the output directory path from the context.
pub fn get_out_dir(ctx: &OrthoHelpContext) -> StepResult<Utf8PathBuf> {
    let out_dir = ctx
        .out_dir
        .with_ref(|dir| {
            Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
                .map_err(|p| format!("non-UTF-8 path: {}", p.display()))
        })
        .ok_or_else(|| "out_dir should be set".to_owned())??;
    Ok(out_dir)
}

/// Gets the workspace root path from the context.
pub fn get_workspace_root(ctx: &OrthoHelpContext) -> StepResult<Utf8PathBuf> {
    ctx.workspace_root
        .with_ref(Clone::clone)
        .ok_or_else(|| "workspace_root should be set".into())
}

pub(super) fn resolve_target_dir(workspace_root: &Utf8Path) -> Utf8PathBuf {
    std::env::var("CARGO_TARGET_DIR").map_or_else(
        |_| workspace_root.join("target"),
        |v| {
            let p = Utf8PathBuf::from(&v);
            if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            }
        },
    )
}

pub(super) fn scenario_target_dir(ctx: &OrthoHelpContext) -> StepResult<Utf8PathBuf> {
    let workspace_root = get_workspace_root(ctx)?;
    let out_dir = get_out_dir(ctx)?;
    let scenario_id = out_dir
        .file_name()
        .ok_or("temporary output directory should have a final path component")?;
    Ok(resolve_target_dir(&workspace_root)
        .join("orthohelp-bdd")
        .join(scenario_id))
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
