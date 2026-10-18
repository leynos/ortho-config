//! Deterministic regressions for documentation subprocess supervision.

#[path = "process_runner_capture_tests.rs"]
mod capture_tests;
#[path = "process_runner_deadline_tests.rs"]
mod deadline_tests;
#[path = "process_runner_test_probes.rs"]
mod probes;

use super::spawn_guard::SpawnGuard;
use super::{
    Operation, Policy, RunHooks, cleanup_incomplete, deadline_exceeded, run_with_hooks,
    run_with_observer, run_with_policy,
};
use anyhow::{Context, Result, anyhow, ensure};
use process_wrap::std::{ChildWrapper, CommandWrap, CommandWrapper};
use std::io;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[test]
fn operation_policies_select_distinct_budgets() {
    let build = Policy::build();
    let execution = Policy::execution();
    let discovery = Policy::discovery();

    assert!(build.timeout() > execution.timeout());
    assert!(execution.timeout() > discovery.timeout());
    assert_eq!(build.output_limit(), 1024 * 1024);
    assert_eq!(build.cleanup_timeout(), Duration::from_secs(2));
    assert_eq!(
        execution.with_timeout(Duration::from_millis(7)).timeout(),
        Duration::from_millis(7)
    );
}

#[test]
fn timeout_diagnostic_keeps_bounded_output_and_truncation_evidence() -> Result<()> {
    let mut command = probe_command("probes::output_then_sleep_probe")?;
    let error = run_with_policy(
        &mut command,
        Operation("capture timeout probe"),
        short_policy().with_output_limit(64),
    )
    .expect_err("the sleeping output probe should time out");
    let diagnostic = format!("{error:#}");

    ensure!(deadline_exceeded(&error), "timeout lost its typed cause");
    ensure!(
        !cleanup_incomplete(&error),
        "cleanup was unexpectedly incomplete"
    );
    ensure!(
        diagnostic.contains("capture timeout probe: subprocess timed out"),
        "timeout diagnostic missing: {diagnostic}"
    );
    ensure_timeout_stream(&diagnostic, "stdout", "STDOUT_TIMEOUT_SENTINEL")?;
    ensure_timeout_stream(&diagnostic, "stderr", "STDERR_TIMEOUT_SENTINEL")?;
    ensure!(
        diagnostic.contains("correlated admission telemetry=unavailable"),
        "admission telemetry availability missing: {diagnostic}"
    );
    Ok(())
}

#[test]
fn wait_failure_remains_primary_when_tree_cleanup_also_fails() -> Result<()> {
    let mut command = probe_command("probes::sleep_probe")?;
    let mut fail_first_wait = true;
    let error = run_with_hooks(
        &mut command,
        Operation("injected wait failure"),
        short_policy(),
        RunHooks {
            observer: |_: &[u8]| {},
            poll: |child: &mut dyn ChildWrapper| {
                if fail_first_wait {
                    fail_first_wait = false;
                    Err(io::Error::other("injected primary wait error"))
                } else {
                    child.try_wait()
                }
            },
            kill: |child: &mut dyn ChildWrapper| {
                child.start_kill()?;
                Err(io::Error::other("injected cleanup error"))
            },
        },
    )
    .expect_err("the injected wait error should be returned");
    let diagnostic = format!("{error:#}");

    ensure!(
        diagnostic.contains("injected primary wait error"),
        "primary wait error missing: {diagnostic}"
    );
    ensure!(
        diagnostic.contains("tree termination failed: injected cleanup error"),
        "cleanup error missing: {diagnostic}"
    );
    ensure!(
        cleanup_incomplete(&error),
        "cleanup was not marked incomplete"
    );
    ensure!(
        error.root_cause().to_string() == "injected primary wait error",
        "primary error changed: {}",
        error.root_cause()
    );
    Ok(())
}

#[test]
fn tree_completion_terminates_descendants_that_keep_output_pipes_open() -> Result<()> {
    let mut command = probe_command("probes::descendants_hold_pipe_probe")?;
    let started = Instant::now();
    let output = run_with_policy(
        &mut command,
        Operation("terminate pipe-holding descendant"),
        Policy::execution()
            .with_timeout(Duration::from_secs(5))
            .with_cleanup_timeout(Duration::from_secs(1)),
    )?;
    let elapsed = started.elapsed();

    ensure!(
        output.status.success(),
        "the root probe did not exit successfully: {}",
        output.status
    );
    ensure!(
        elapsed < Duration::from_secs(2),
        "tree cleanup took {elapsed:?}"
    );
    Ok(())
}

#[test]
fn closed_pipe_writer_is_gone_before_success_and_external_process_survives() -> Result<()> {
    let coordination = tempfile::tempdir().context("create private probe directory")?;
    let coordination_path = coordination
        .path()
        .to_str()
        .ok_or_else(|| anyhow!("probe directory path is not UTF-8"))?;
    let coordination_dir =
        cap_std::fs_utf8::Dir::open_ambient_dir(coordination_path, cap_std::ambient_authority())?;
    let mut sentinel_command = probe_command("probes::external_sentinel_probe")?;
    sentinel_command.stdout(Stdio::null()).stderr(Stdio::null());
    let mut sentinel = sentinel_command
        .spawn()
        .context("start external sentinel probe")?;

    let mut command = probe_command("probes::closed_pipe_outer_probe")?;
    command.env("ORTHO452_TREE_PROBE_DIRECTORY", coordination_path);
    let started = Instant::now();
    let outcome = run_with_policy(
        &mut command,
        Operation("verify closed-pipe descendant completion"),
        Policy::execution()
            .with_timeout(Duration::from_secs(5))
            .with_cleanup_timeout(Duration::from_secs(1)),
    );
    let elapsed = started.elapsed();
    let observations = (|| -> Result<_> {
        let writer_process_id = coordination_dir
            .read_to_string("writer-pid")?
            .parse::<u32>()?;
        Ok((
            super::tree::process_is_active(writer_process_id)?,
            super::tree::process_is_active(sentinel.id())?,
            coordination_dir.try_exists("writer-finished")?,
        ))
    })();
    let sentinel_kill = sentinel.kill();
    let sentinel_wait = sentinel.wait();

    let (writer_active, sentinel_active, writer_finished) = observations?;
    let output = outcome?;
    ensure!(output.status.success(), "closed-pipe root probe failed");
    ensure!(
        !writer_active,
        "runner returned while the writer descendant was still active"
    );
    ensure!(
        !writer_finished,
        "writer completed naturally before tree cleanup"
    );
    ensure!(
        sentinel_active,
        "tree cleanup terminated the unrelated sentinel process"
    );
    ensure!(
        elapsed < Duration::from_secs(2),
        "runner waited for the writer's natural exit: {elapsed:?}"
    );
    sentinel_kill.context("terminate the external sentinel probe")?;
    sentinel_wait.context("reap the external sentinel probe")?;
    Ok(())
}

#[test]
fn incomplete_runner_cleanup_reports_cancelled_readers() -> Result<()> {
    let mut command = probe_command("probes::sleep_probe")?;
    let error = run_with_hooks(
        &mut command,
        Operation("report bounded reader cancellation"),
        Policy::execution()
            .with_timeout(Duration::from_millis(100))
            .with_cleanup_timeout(Duration::from_millis(200)),
        RunHooks {
            observer: |_: &[u8]| {},
            poll: |child: &mut dyn ChildWrapper| child.try_wait(),
            kill: |_child: &mut dyn ChildWrapper| Ok(()),
        },
    )
    .expect_err("the live probe should exceed the operation deadline");
    let diagnostic = format!("{error:#}");

    ensure!(deadline_exceeded(&error), "timeout lost its typed cause");
    ensure!(
        cleanup_incomplete(&error),
        "cleanup completion was not marked"
    );
    ensure!(
        diagnostic.contains("report bounded reader cancellation: subprocess timed out"),
        "timeout diagnostic missing: {diagnostic}"
    );
    ensure!(
        diagnostic.contains("stdout reader was cancelled before EOF"),
        "stdout cancellation evidence missing: {diagnostic}"
    );
    ensure!(
        diagnostic.contains("stderr reader was cancelled before EOF"),
        "stderr cancellation evidence missing: {diagnostic}"
    );
    Ok(())
}

#[test]
fn stdout_observer_receives_bytes_beyond_the_capture_limit() -> Result<()> {
    let mut command = probe_command("probes::bounded_success_output_probe")?;
    let observed = Arc::new(Mutex::new(Vec::new()));
    let observer_output = Arc::clone(&observed);
    let output = run_with_observer(
        &mut command,
        Operation("observe complete output"),
        Policy::execution()
            .with_timeout(Duration::from_secs(5))
            .with_output_limit(32),
        move |chunk| {
            observer_output
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(chunk);
        },
    )?;

    ensure!(output.status.success(), "probe process failed");
    ensure!(
        output.stdout.len() == 32,
        "retained output was not bounded to 32 bytes"
    );
    ensure!(
        observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            >= 4096,
        "observer did not receive output beyond the capture limit"
    );
    Ok(())
}

#[test]
fn wrapper_failure_kills_only_the_owned_root_and_marks_cleanup_unconfirmed() -> Result<()> {
    let probe = probe_command("probes::bounded_success_output_probe")?;
    let mut wrapped_probe = CommandWrap::from(probe);
    let guard = SpawnGuard::new();
    guard.install(&mut wrapped_probe);
    wrapped_probe.wrap(FailingTreeWrapper);

    let error = super::finish_wrapped_spawn(&mut wrapped_probe, &guard)
        .expect_err("the injected process-tree wrapper should fail");

    ensure!(
        guard.root_child_seen(),
        "spawn wrapper missed the owned root"
    );
    ensure!(
        guard.kill_attempted(),
        "spawn guard did not request root termination"
    );
    ensure!(
        cleanup_incomplete(&error),
        "wrapper failure was not marked incomplete"
    );
    Ok(())
}

#[derive(Debug)]
struct FailingTreeWrapper;

impl CommandWrapper for FailingTreeWrapper {
    fn wrap_child(
        &mut self,
        _child: Box<dyn ChildWrapper>,
        _command: &CommandWrap,
    ) -> io::Result<Box<dyn ChildWrapper>> {
        Err(io::Error::other("injected tree wrapper failure"))
    }
}

fn short_policy() -> Policy {
    Policy::execution()
        .with_timeout(Duration::from_millis(150))
        .with_cleanup_timeout(Duration::from_millis(700))
}

fn ensure_timeout_stream(diagnostic: &str, stream: &str, sentinel: &str) -> Result<()> {
    ensure!(
        diagnostic.contains(&format!("{stream}: bytes_seen=")),
        "{stream} byte-count evidence missing: {diagnostic}"
    );
    ensure!(
        diagnostic.contains("bytes_captured=64, truncated=true"),
        "{stream} truncation evidence missing: {diagnostic}"
    );
    ensure!(
        diagnostic.contains(sentinel),
        "{stream} output sentinel missing: {diagnostic}"
    );
    Ok(())
}

fn probe_command(test_name: &str) -> Result<Command> {
    let executable = std::env::current_exe().context("test executable should have a path")?;
    let mut command = Command::new(executable);
    command.args([
        "--ignored",
        "--quiet",
        "--nocapture",
        "--test-threads=1",
        test_name,
    ]);
    Ok(command)
}
