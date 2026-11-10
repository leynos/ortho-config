//! The second-process harness shared by the identifier-artefact lease tests.
//!
//! The regressions in this target all start a second copy of the test binary,
//! give it a scratch directory, and then observe what it does through marker
//! files. They differ in what the second process is asked to do and in how it
//! is torn down, so the spawning, polling and teardown live here rather than
//! being repeated in each regression.
//!
//! Teardown is deliberately two-staged. On Unix the helper leads its own
//! process group, so killing it reaches the helper alone; the nested build it
//! started has its own group. [`Helper::drop`] therefore records the helper's
//! descendants before it kills anything, then removes both, so a failing
//! regression cannot leave a compiler tree running against the target. On
//! Windows the helper's Job Object owns that tree, and killing the helper is
//! enough.

use super::identifier_artefact_e2e_support::snapshot_descendants;
use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs::Dir};
#[cfg(windows)]
use process_wrap::std::JobObject;
#[cfg(unix)]
use process_wrap::std::ProcessGroup;
use process_wrap::std::{ChildWrapper, CommandWrap};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// Environment variable naming the scratch directory shared with the child.
pub(super) const SCRATCH_VARIABLE: &str = "IDENTIFIER_LEASE_SCRATCH";

/// How long the second process may take to discover and announce the target.
///
/// Discovery runs `cargo metadata`, which can queue behind a busy package-cache
/// lock on a shared host, so this is deliberately much longer than the work
/// itself.
pub(super) const HELPER_STARTUP: Duration = Duration::from_secs(120);

/// How long the second process may take to clean, rebuild, and report once the
/// lease is released.
pub(super) const HELPER_DEADLINE: Duration = Duration::from_secs(900);

/// The envelope the second process grants itself, inside [`HELPER_DEADLINE`].
///
/// Only the helper supervises its nested Cargo invocations: the runner gives
/// each child its own process group, so the parent's kill reaches the helper
/// alone. If the parent's bound expired while Cargo was still building, the
/// kill would remove the one process that bounds Cargo and leave it running
/// unsupervised. Capping the helper's own envelope well inside the parent's
/// bound means its supervised Cargo reaches *its* deadline first, is killed and
/// reaped by the runner that owns it, and the helper reports the failure and
/// exits before the parent stops waiting.
pub(super) const HELPER_WORK_BOUND: Duration = Duration::from_secs(600);

/// How often the parent polls the second process and its marker files.
pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// How long the parent waits for a killed second process to disappear.
pub(super) const TEARDOWN: Duration = Duration::from_secs(5);

/// The file a nested writer appends to inside the leased target.
///
/// Its length is the heartbeat. Each beat is a single one-byte write to an
/// unbuffered handle, so a reader observes a size that only grows while the
/// writer lives; a written counter could be caught half-overwritten.
pub(super) const HEARTBEAT_FILE: &str = ".identifier-artefact.heartbeat";

/// How often a nested writer appends one byte to the heartbeat.
pub(super) const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(50);

/// The second process, owned so a failure cannot leave it running.
pub(super) struct Helper {
    child: Box<dyn ChildWrapper>,
    /// Set once the child has been reaped, so teardown does not signal a
    /// process group whose leader's identifier the kernel may have reused.
    status: Option<ExitStatus>,
}

impl Helper {
    /// Start `test_name` from this binary as the root of a tree this test can
    /// kill.
    ///
    /// `test_name` is the child test's name, qualified by its module path, as
    /// `--exact` requires.
    pub(super) fn spawn_for(test_name: &str, scratch: &Path) -> Result<Self> {
        Self::spawn_for_with_stdin(test_name, scratch, None)
    }

    /// Start `test_name`, optionally handing it an inherited handle on stdin.
    ///
    /// The lease-inheritance regression passes a duplicate of the target lease
    /// this way: the child keeps the lease held after this process drops its own
    /// handle, which is exactly the ordering an interrupted nested build needs.
    pub(super) fn spawn_for_with_stdin(
        test_name: &str,
        scratch: &Path,
        stdin: Option<Stdio>,
    ) -> Result<Self> {
        let mut command = child_test_command(test_name, scratch)?;
        if let Some(handle) = stdin {
            command.stdin(handle);
        }
        let mut wrapped = CommandWrap::from(command);
        #[cfg(unix)]
        wrapped.wrap(ProcessGroup::leader());
        #[cfg(windows)]
        wrapped.wrap(JobObject);
        let child = wrapped.spawn().context("start the second test process")?;
        Ok(Self {
            child,
            status: None,
        })
    }

    /// The second process's identifier, while it is alive.
    ///
    /// Only Unix teardown needs it: Windows relies on the helper's Job Object
    /// and never records descendants.
    #[cfg(unix)]
    pub(super) fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Reap the child if it has exited, remembering the result.
    pub(super) fn try_status(&mut self) -> Result<Option<ExitStatus>> {
        if self.status.is_none() {
            self.status = self.child.try_wait()?;
        }
        Ok(self.status)
    }

    /// Wait for the child, refusing to wait unboundedly for a wedged one.
    pub(super) fn await_exit(&mut self, limit: Duration) -> Result<ExitStatus> {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = self.try_status()? {
                return Ok(status);
            }
            ensure!(
                Instant::now() < deadline,
                "the second test process did not exit within {limit:?}",
            );
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Kill the helper without waiting for the tree it may have started.
    ///
    /// On Unix the helper leads its own process group, so this reaches the
    /// helper alone; its nested build survives. On Windows the helper's Job
    /// Object closes with it, so the nested build is terminated too.
    pub(super) fn kill_alone(&mut self) -> Result<()> {
        ensure!(
            self.status.is_none(),
            "the second test process had already exited before it was killed",
        );
        self.child
            .start_kill()
            .context("kill the second test process")?;
        let _status = self.await_exit(TEARDOWN)?;
        Ok(())
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        if self.status.is_some() {
            return;
        }
        // Record the descendants while their parent links still exist, then
        // remove the helper and everything it left behind.
        let descendants = snapshot_descendants(self.child.id()).unwrap_or_default();
        let _kill = self.child.start_kill();
        let _terminated = descendants.terminate();
        let deadline = Instant::now() + TEARDOWN;
        while Instant::now() < deadline
            && self.child.try_wait().is_ok_and(|status| status.is_none())
        {
            std::thread::sleep(POLL_INTERVAL);
        }
    }
}

/// Wait for the second process to write `marker`, failing if it exits first.
pub(super) fn wait_for_marker(
    helper: &mut Helper,
    scratch: &Dir,
    marker: &str,
    limit: Duration,
) -> Result<()> {
    let deadline = Instant::now() + limit;
    loop {
        ensure!(
            helper.try_status()?.is_none(),
            "the second test process exited before writing the `{marker}` marker",
        );
        if scratch.exists(marker) {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "timed out waiting for the second test process to write `{marker}`",
        );
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// The heartbeat's current length in the leased target, or zero when absent.
pub(super) fn heartbeat_length(target: &Dir) -> u64 {
    target
        .metadata(HEARTBEAT_FILE)
        .map_or(0, |metadata| metadata.len())
}

/// Wait until the nested writer's heartbeat has reached `minimum` bytes.
///
/// The wait fails as soon as the helper exits, so a helper that died before
/// starting its writer is reported as that, rather than as a timeout.
pub(super) fn wait_for_heartbeat(
    helper: &mut Helper,
    target: &Dir,
    minimum: u64,
    limit: Duration,
) -> Result<()> {
    let deadline = Instant::now() + limit;
    loop {
        ensure!(
            helper.try_status()?.is_none(),
            "the second test process exited before its nested writer reached \
             {minimum} heartbeat bytes in the leased target",
        );
        if heartbeat_length(target) >= minimum {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "timed out waiting for the nested writer's heartbeat in the leased target",
        );
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// The heartbeat length after `span`, so a caller can compare the two samples.
pub(super) fn heartbeat_after(target: &Dir, span: Duration) -> u64 {
    std::thread::sleep(span);
    heartbeat_length(target)
}

/// Read one `name=value` line from the second process's report.
pub(super) fn report_field<'a>(report: &'a str, name: &str) -> Result<&'a str> {
    report
        .lines()
        .find_map(|line| {
            line.strip_prefix(name)
                .and_then(|rest| rest.strip_prefix('='))
        })
        .with_context(|| format!("the second test process's report omitted `{name}`"))
}

/// The scratch directory named by the parent, as a path.
pub(super) fn required_scratch_path() -> Result<PathBuf> {
    let path = std::env::var_os(SCRATCH_VARIABLE)
        .with_context(|| format!("{SCRATCH_VARIABLE} should name the scratch directory"))?;
    Ok(PathBuf::from(path))
}

/// Open the scratch directory named by the parent.
pub(super) fn required_scratch() -> Result<Dir> {
    let path = required_scratch_path()?;
    Dir::open_ambient_dir(&path, ambient_authority())
        .with_context(|| format!("open the scratch directory {}", path.display()))
}

/// Run `test_name` from this binary as an ignored child test.
///
/// `test_name` is qualified by its module path, as `--exact` requires.
pub(super) fn child_test_command(test_name: &str, scratch: &Path) -> Result<Command> {
    let executable = std::env::current_exe().context("the test binary should have a path")?;
    let mut command = Command::new(executable);
    command
        .args([
            "--ignored",
            "--exact",
            "--quiet",
            "--nocapture",
            "--test-threads=1",
            test_name,
        ])
        .env(SCRATCH_VARIABLE, scratch);
    Ok(command)
}
