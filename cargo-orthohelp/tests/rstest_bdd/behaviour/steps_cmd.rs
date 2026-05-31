//! Command execution step definitions for `cargo-orthohelp` behavioural tests.

use std::process::Command;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{given, when};

use super::steps::{OrthoHelpContext, StepResult};
use super::steps_cache::{CACHE_ARGS, is_not_found_kind, record_cache_state};
use crate::fixtures;

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
    let target_dir = resolve_target_dir(&workspace_root);
    match Dir::open_ambient_dir(target_dir.as_path(), ambient_authority()) {
        Ok(root_dir) => {
            if let Err(err) = root_dir.remove_dir_all("orthohelp")
                && !is_not_found_kind(&err)
            {
                return Err(format!("remove orthohelp cache failed: {err}").into());
            }
        }
        Err(err) if is_not_found_kind(&err) => {}
        Err(err) => return Err(format!("remove orthohelp cache failed: {err}").into()),
    }
    orthohelp_context.cache_ir_path.clear();
    orthohelp_context.cache_ir_content.clear();
    Ok(())
}

#[expect(
    clippy::panic_in_result_fn,
    reason = "BDD step helpers use assertions for scenario failure diagnostics."
)]
fn run_orthohelp_with_cache_args(ctx: &mut OrthoHelpContext) -> StepResult<()> {
    let output = run_orthohelp(ctx, CACHE_ARGS)?;
    assert!(output.status.success(), "cargo-orthohelp should succeed");
    ctx.last_output.set(output);
    Ok(())
}

#[when("I run cargo-orthohelp with cache for the fixture")]
fn run_with_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    run_orthohelp_with_cache_args(orthohelp_context)?;
    record_cache_state(orthohelp_context)?;
    Ok(())
}

#[when("I rerun cargo-orthohelp with cache for the fixture")]
fn rerun_with_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    // Ensure filesystem timestamp granularity distinguishes the cache file mtime.
    std::thread::sleep(Duration::from_secs(1));
    run_orthohelp_with_cache_args(orthohelp_context)
}

#[when("I run cargo-orthohelp with no-build for the fixture")]
fn run_with_no_build(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output = run_orthohelp(
        orthohelp_context,
        &[
            "--no-build",
            "--package",
            "orthohelp_fixture",
            "--locale",
            "en-US",
        ],
    )?;
    orthohelp_context.last_output.set(output);
    Ok(())
}

#[when("I run cargo-orthohelp with format ir for the fixture")]
fn run_with_format_ir(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
    let output = run_orthohelp(
        orthohelp_context,
        &[
            "--format",
            "ir",
            "--package",
            "orthohelp_fixture",
            "--locale",
            "en-US",
        ],
    )?;
    assert!(
        output.status.success(),
        "cargo-orthohelp should succeed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    orthohelp_context.last_output.set(output);
    Ok(())
}

/// Runs cargo-orthohelp with the given arguments.
pub fn run_orthohelp(ctx: &OrthoHelpContext, args: &[&str]) -> StepResult<std::process::Output> {
    let exe = fixtures::cargo_orthohelp_exe()?;
    let workspace_root = get_workspace_root(ctx)?;
    let out_dir = get_out_dir(ctx)?;
    let mut command = Command::new(exe.as_str());
    command
        .current_dir(workspace_root.as_str())
        .arg("orthohelp")
        .arg("--out-dir")
        .arg(out_dir.as_str())
        .args(args);
    Ok(command.output()?)
}
