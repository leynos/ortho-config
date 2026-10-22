//! Reap only the Cargo child created by the bridge build helper.
//!
//! The outer process supervisor owns hard-deadline process-tree cleanup; this
//! guard provides bounded best-effort cleanup for an earlier protocol failure.

use std::io;
use std::process::{Child, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

use super::telemetry;

/// Maximum time to wait for the owned Cargo process to exit after killing it.
const CHILD_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
/// Interval between non-blocking child-exit checks during bounded cleanup.
const CLEANUP_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Owns the direct Cargo child until it is reaped or cleanup is attempted.
pub(super) struct OwnedChild<Process: ChildCleanup = Child> {
    /// Child process created for this bridge build.
    pub(super) child: Process,
    /// Whether a successful wait or poll already reaped the process.
    reaped: bool,
}

impl<Process: ChildCleanup> OwnedChild<Process> {
    /// Wrap a newly spawned child that has not yet been reaped.
    pub(super) const fn new(child: Process) -> Self {
        Self {
            child,
            reaped: false,
        }
    }

    /// Kill and reap Cargo within the fixed cleanup window, returning evidence.
    pub(super) fn stop_bounded(&mut self) -> String {
        self.stop_bounded_with(CHILD_CLEANUP_TIMEOUT, CLEANUP_POLL_INTERVAL)
    }

    /// Apply an injected cleanup window for deterministic ownership tests.
    fn stop_bounded_with(&mut self, timeout: Duration, poll_interval: Duration) -> String {
        let mut evidence = Vec::new();
        if let Err(error) = self.child.kill() {
            evidence.push(format!(
                "owned Cargo termination request failed: {:?}",
                error.kind()
            ));
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .unwrap_or_else(Instant::now);

        while Instant::now() < deadline {
            match self.try_reap(&mut evidence) {
                ReapResult::Reaped | ReapResult::Failed => return evidence.join("; "),
                ReapResult::StillRunning => {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    thread::sleep(poll_interval.min(remaining));
                }
            }
        }
        evidence.push("owned Cargo did not exit before bounded cleanup expired".to_owned());
        evidence.join("; ")
    }

    /// Poll once, recording whether Cargo exited or whether reaping failed.
    fn try_reap(&mut self, evidence: &mut Vec<String>) -> ReapResult {
        match self.child.try_reap() {
            Ok(true) => {
                self.reaped = true;
                evidence.push(
                    "owned Cargo exited; descendant writer lifetime remains unconfirmed".to_owned(),
                );
                ReapResult::Reaped
            }
            Ok(false) => ReapResult::StillRunning,
            Err(error) => {
                evidence.push(format!("owned Cargo reap failed: {:?}", error.kind()));
                ReapResult::Failed
            }
        }
    }
}

/// Operations needed to terminate and observe one owned child process.
pub(super) trait ChildCleanup {
    /// Request termination of the owned child.
    fn kill(&mut self) -> io::Result<()>;
    /// Return whether a non-blocking poll reaped the owned child.
    fn try_reap(&mut self) -> io::Result<bool>;
}

impl ChildCleanup for Child {
    fn kill(&mut self) -> io::Result<()> {
        Self::kill(self)
    }

    fn try_reap(&mut self) -> io::Result<bool> {
        Self::try_wait(self).map(|status| status.is_some())
    }
}

impl OwnedChild<Child> {
    /// Wait for Cargo to finish and record that its process has been reaped.
    pub(super) fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self.child.wait()?;
        self.reaped = true;
        Ok(status)
    }
}

/// Outcome of a non-blocking attempt to reap the owned Cargo process.
enum ReapResult {
    /// Cargo exited and its status was collected.
    Reaped,
    /// Cargo remains alive and can be checked again before the deadline.
    StillRunning,
    /// The operating system could not report Cargo's process state.
    Failed,
}

impl<Process: ChildCleanup> Drop for OwnedChild<Process> {
    fn drop(&mut self) {
        if self.reaped {
            return;
        }
        if let Err(error) = self.child.kill() {
            telemetry::owned_child_kill_failed(error.kind());
        }
    }
}

#[cfg(test)]
#[path = "bridge_build_cleanup_tests.rs"]
mod tests;
