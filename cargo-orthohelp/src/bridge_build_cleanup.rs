//! Reap only the Cargo child created by the bridge build helper.
//!
//! The outer process supervisor owns hard-deadline process-tree cleanup; this
//! guard provides bounded best-effort cleanup for an earlier protocol failure.

use std::io;
use std::process::{Child, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

/// Maximum time to wait for the owned Cargo process to exit after killing it.
const CHILD_CLEANUP_TIMEOUT: Duration = Duration::from_secs(2);
/// Interval between non-blocking child-exit checks during bounded cleanup.
const CLEANUP_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Owns the direct Cargo child until it is reaped or cleanup is attempted.
pub(super) struct OwnedChild {
    /// Child process created for this bridge build.
    pub(super) child: Child,
    /// Whether a successful wait or poll already reaped the process.
    reaped: bool,
}

impl OwnedChild {
    /// Wrap a newly spawned child that has not yet been reaped.
    pub(super) const fn new(child: Child) -> Self {
        Self {
            child,
            reaped: false,
        }
    }

    /// Wait for Cargo to finish and record that its process has been reaped.
    pub(super) fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self.child.wait()?;
        self.reaped = true;
        Ok(status)
    }

    /// Kill and reap Cargo within the fixed cleanup window, returning evidence.
    pub(super) fn stop_bounded(&mut self) -> String {
        let mut evidence = Vec::new();
        if let Err(error) = self.child.kill() {
            evidence.push(format!("owned Cargo termination request failed: {error}"));
        }
        let deadline = Instant::now()
            .checked_add(CHILD_CLEANUP_TIMEOUT)
            .unwrap_or_else(Instant::now);

        while Instant::now() < deadline {
            match self.try_reap(&mut evidence) {
                ReapResult::Reaped | ReapResult::Failed => return evidence.join("; "),
                ReapResult::StillRunning => thread::sleep(CLEANUP_POLL_INTERVAL),
            }
        }
        evidence.push("owned Cargo did not exit before bounded cleanup expired".to_owned());
        evidence.join("; ")
    }

    /// Poll once, recording whether Cargo exited or whether reaping failed.
    fn try_reap(&mut self, evidence: &mut Vec<String>) -> ReapResult {
        match self.child.try_wait() {
            Ok(Some(_)) => {
                self.reaped = true;
                evidence.push(
                    "owned Cargo exited; descendant writer lifetime remains unconfirmed".to_owned(),
                );
                ReapResult::Reaped
            }
            Ok(None) => ReapResult::StillRunning,
            Err(error) => {
                evidence.push(format!("owned Cargo reap failed: {error}"));
                ReapResult::Failed
            }
        }
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

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.reaped {
            return;
        }
        if let Err(error) = self.child.kill() {
            tracing::warn!(error = %error, "failed to terminate owned Cargo child");
        }
    }
}
