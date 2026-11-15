//! Command execution step definitions for `cargo-orthohelp` behavioural tests.
//!
//! Implements `given`/`when` steps that run `cargo-orthohelp` as a subprocess
//! and manage per-scenario filesystem state:
//!
//! - **`the output directory is empty`** — verifies the scenario's temporary
//!   output directory exists and contains no files.
//! - **`the orthohelp cache is empty`** — removes the on-disk cache directory
//!   under the scenario target dir.
//! - **`I run cargo-orthohelp with cache for the fixture`** — runs with
//!   [`CACHE_ARGS`] and records cache state.
//! - **`I rerun cargo-orthohelp with cache for the fixture`** — reruns with
//!   cache args (no cache-state recording).
//! - **`I run cargo-orthohelp with --no-build`** — verifies the no-build path.
//! - **`I run cargo-orthohelp with format ir`** — asserts the `ir` format
//!   succeeds.
//!
//! Exposes [`run_orthohelp`], the public helper that builds and executes the
//! `cargo-orthohelp` command for a given scenario context and argument slice.

use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use rstest_bdd_macros::{given, when};
use std::process::Command;
use std::time::{Duration, Instant};

use super::steps::{
    OrthoHelpContext, StepResult, get_out_dir, get_workspace_root, scenario_target_dir,
};
use super::steps_cache::{CACHE_ARGS, is_not_found_kind, record_cache_state};
use crate::fixtures;

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
    let target_dir = scenario_target_dir(orthohelp_context)?;
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
    let (cache_path, content) = record_cache_state(orthohelp_context)?;
    orthohelp_context.cache_ir_path.set(cache_path);
    orthohelp_context.cache_ir_content.set(content);
    Ok(())
}

#[when("I rerun cargo-orthohelp with cache for the fixture")]
fn rerun_with_cache(orthohelp_context: &mut OrthoHelpContext) -> StepResult<()> {
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
    let target_dir = scenario_target_dir(ctx)?;
    let mut command = Command::new(exe.as_str());
    crate::cargo_environment::CargoEnvironment::build().apply(&mut command);
    command
        .current_dir(workspace_root.as_str())
        .env("CARGO_TARGET_DIR", target_dir.as_str())
        .arg("orthohelp")
        .arg("--out-dir")
        .arg(out_dir.as_str())
        .args(args);
    let deadline = ctx
        .deadline
        .with_ref(|deadline| *deadline)
        .ok_or("BDD scenario deadline should be set")?;
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return Err("BDD scenario deadline elapsed before cargo-orthohelp execution".into());
    }
    let default_policy = if args.contains(&"--no-build") {
        crate::process_runner_execution_policy::policy()
    } else {
        bdd_build_policy()
    };
    let timeout = remaining
        .saturating_sub(default_policy.cleanup_timeout())
        .min(default_policy.timeout());
    if timeout.is_zero() {
        return Err("BDD scenario deadline leaves no time for subprocess cleanup".into());
    }
    crate::process_runner::run_with_policy(
        &mut command,
        crate::process_runner::Operation("run cargo-orthohelp BDD scenario"),
        default_policy.bounded_by(timeout),
    )
    .map_err(|error| classify_runner_error(&error))
}

const fn bdd_build_policy() -> crate::process_runner::Policy {
    let policy = crate::process_runner::Policy::build();
    let timeout = Duration::from_secs(super::steps::SCENARIO_TIMEOUT_SECS)
        .saturating_sub(policy.cleanup_timeout());
    crate::process_runner::Policy::new(
        policy.kind_name(),
        timeout,
        policy.cleanup_timeout(),
        policy.output_limit(),
    )
}

fn classify_runner_error(error: &anyhow::Error) -> super::steps::StepError {
    let category = if crate::process_runner::deadline_exceeded(error) {
        "subprocess deadline exceeded"
    } else if crate::process_runner::cleanup_incomplete(error) {
        "subprocess cleanup was not confirmed"
    } else {
        "subprocess failed"
    };
    format!("cargo-orthohelp BDD {category}: {error:#}").into()
}

#[cfg(test)]
mod tests {
    //! Tests for the BDD build subprocess budget.

    use super::bdd_build_policy;
    use crate::rstest_bdd_support::behaviour::steps::SCENARIO_TIMEOUT_SECS;
    use std::time::Duration;

    #[test]
    fn bdd_build_and_cleanup_budgets_fit_the_scenario_deadline() {
        let policy = bdd_build_policy();

        assert_eq!(policy.kind_name(), "build");
        assert_eq!(
            policy.timeout() + policy.cleanup_timeout(),
            Duration::from_secs(SCENARIO_TIMEOUT_SECS)
        );
    }
}
