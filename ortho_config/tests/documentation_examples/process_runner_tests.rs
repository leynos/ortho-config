//! Deterministic regressions for documentation subprocess supervision.

#[path = "process_runner_capture_tests.rs"]
mod capture_tests;
#[path = "process_runner_deadline_tests.rs"]
mod deadline_tests;
#[path = "process_runner_test_probes.rs"]
mod probes;
#[cfg(target_os = "linux")]
use super::tree::read_proc_stat;
#[cfg(target_os = "linux")]
#[path = "process_runner_tree_unix_tests.rs"]
mod unix_tree_tests;

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
fn operation_policies_select_distinct_timeouts() {
    let build = Policy::build();
    let execution = crate::process_runner_execution_policy::policy();
    let discovery = Policy::discovery();

    assert_eq!(build.timeout(), Duration::from_secs(596));
    assert!(build.timeout() > execution.timeout());
    assert!(execution.timeout() > discovery.timeout());
}

#[test]
fn operation_policies_preserve_capture_and_cleanup_limits() {
    let build = Policy::build();
    let execution = crate::process_runner_execution_policy::policy();
    assert_eq!(build.output_limit(), 1024 * 1024);
    assert_eq!(build.cleanup_timeout(), Duration::from_secs(2));
    assert_eq!(execution.output_limit(), 1024 * 1024);
    assert_eq!(execution.cleanup_timeout(), Duration::from_secs(2));
}

#[test]
fn bounded_build_policy_keeps_its_other_limits() {
    let build = Policy::build();
    assert_eq!(
        execution_policy(Duration::from_millis(7), Duration::from_secs(2), 1024).timeout(),
        Duration::from_millis(7)
    );

    let bounded = build.bounded_by(Duration::from_secs(17));
    assert_eq!(bounded.timeout(), Duration::from_secs(17));
    assert_eq!(bounded.cleanup_timeout(), build.cleanup_timeout());
    assert_eq!(bounded.output_limit(), build.output_limit());
    assert_eq!(bounded.kind_name(), build.kind_name());
    assert_eq!(
        build.bounded_by(Duration::from_secs(700)).timeout(),
        build.timeout()
    );
}

#[test]
fn timeout_diagnostic_keeps_bounded_output_and_truncation_evidence() -> Result<()> {
    let mut command = probe_command("probes::output_then_sleep_probe")?;
    let error = run_with_policy(
        &mut command,
        Operation("capture timeout probe"),
        execution_policy(Duration::from_millis(150), Duration::from_millis(700), 64),
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
        execution_policy(Duration::from_secs(5), Duration::from_secs(1), 1024 * 1024),
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
        execution_policy(Duration::from_secs(5), Duration::from_secs(1), 1024 * 1024),
    );
    let elapsed = started.elapsed();
    let observations = (|| -> Result<_> {
        let writer_process_id = coordination_dir
            .read_to_string("writer-pid")?
            .parse::<u32>()?;
        Ok((
            probes::process_is_active(writer_process_id)?,
            probes::process_is_active(sentinel.id())?,
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
        execution_policy(
            Duration::from_millis(100),
            Duration::from_millis(200),
            1024 * 1024,
        ),
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
        execution_policy(Duration::from_secs(5), Duration::from_secs(2), 32),
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

const fn execution_policy(
    timeout: Duration,
    cleanup_timeout: Duration,
    output_limit: usize,
) -> Policy {
    Policy::new("execution", timeout, cleanup_timeout, output_limit)
}

const fn short_policy() -> Policy {
    execution_policy(
        Duration::from_millis(150),
        Duration::from_millis(700),
        1024 * 1024,
    )
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
