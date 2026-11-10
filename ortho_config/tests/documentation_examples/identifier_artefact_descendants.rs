//! Terminate the process tree an interrupted nested build leaves behind.
//!
//! A helper test process runs its nested build through
//! [`super::process_runner`], which gives every child its own process group
//! (Unix) or Job Object (Windows). That is what lets the runner kill a whole
//! build at its deadline. It also means the parent test's kill of the helper
//! reaches the helper alone: a helper that panics, aborts or is killed
//! abruptly leaves Cargo and its compilers running with nothing supervising
//! them.
//!
//! The lease itself stays held by those processes because each nested build
//! inherits a duplicate of the lease handle; see the lease module. This module
//! supplies the other half: a snapshot of the helper's descendants, taken while
//! the parent links still exist, and a bounded termination of the recorded
//! processes. The regression in the lease tests proves both halves: an
//! orphaned writer keeps the lease refused, and terminating it makes the
//! target acquirable again.
//!
//! Windows needs no snapshot. The runner's Job Object is created with
//! `KILL_ON_JOB_CLOSE` and belongs to the helper, so killing the helper makes
//! the kernel terminate the whole tree. The snapshot is empty there, and
//! [`Descendants::terminate`] has nothing left to do.

use anyhow::Result;
#[cfg(unix)]
use anyhow::{Context, ensure};
#[cfg(unix)]
use nix::errno::Errno;
#[cfg(unix)]
use nix::sys::signal::{Signal, kill, killpg};
#[cfg(unix)]
use nix::unistd::{Pid, getpgid};
#[cfg(unix)]
use std::collections::HashMap;
#[cfg(unix)]
use std::process::Command;
#[cfg(unix)]
use std::time::{Duration, Instant};

/// How long the Unix teardown waits for its recorded descendants to disappear.
///
/// A process that has been sent `SIGKILL` cannot refuse, so this only allows
/// for the kernel's scheduling delay before the process becomes unsignalable.
#[cfg(unix)]
const TERMINATION_BOUND: Duration = Duration::from_secs(5);

/// How often the teardown re-checks a recorded descendant.
#[cfg(unix)]
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// The processes that were running beneath the helper at snapshot time.
///
/// Taken before the helper is killed, because a killed process's children are
/// re-parented and can no longer be found by following parent links.
#[derive(Debug, Default)]
pub(crate) struct Descendants {
    /// The recorded descendant process IDs, excluding the helper itself.
    #[cfg(unix)]
    pids: Vec<u32>,
}

impl Descendants {
    /// Whether the snapshot recorded no descendants.
    #[cfg(unix)]
    pub(crate) const fn is_empty(&self) -> bool {
        self.pids.is_empty()
    }

    /// Terminate every recorded descendant, giving up after a fixed bound.
    ///
    /// The helper must already have been killed: that stops new descendants
    /// appearing while this runs. A descendant that leads its own process
    /// group is killed with the group, so a compiler it started after the
    /// snapshot cannot survive it.
    ///
    /// # Errors
    ///
    /// Returns an error when a recorded descendant is still running at the
    /// end of the bound. The lease still protects the target in that case;
    /// this reports the leftover rather than claiming a clean teardown.
    #[cfg(unix)]
    pub(crate) fn terminate(&self) -> Result<()> {
        let deadline = Instant::now() + TERMINATION_BOUND;
        // A refused signal is the reason a descendant survived, so it belongs
        // in the diagnostic. A signal to a process that has already exited
        // answers `ESRCH` and needs no mention: the survivor list is empty then.
        let mut signal_failures = Vec::new();
        loop {
            signal_failures.clear();
            for pid in &self.pids {
                if let Some(error) = terminate_descendant(*pid) {
                    signal_failures.push((*pid, error));
                }
            }
            let survivors = self
                .pids
                .iter()
                .copied()
                .filter(|pid| is_alive(*pid))
                .collect::<Vec<_>>();
            if survivors.is_empty() {
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "nested build descendants survived termination for {TERMINATION_BOUND:?}: \
                 {survivors:?}{}",
                describe_signal_failures(&signal_failures),
            );
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    /// Terminate every recorded descendant.
    ///
    /// Windows records none: the helper's Job Object terminates its own tree
    /// when the helper dies, before this could run.
    #[cfg(not(unix))]
    pub(crate) const fn terminate(&self) -> Result<()> {
        Ok(())
    }
}

/// Record the descendants of `root` while their parent links still exist.
///
/// # Errors
///
/// Returns an error when the process table cannot be read.
#[cfg(unix)]
pub(crate) fn snapshot_descendants(root: u32) -> Result<Descendants> {
    let output = Command::new("ps")
        .args(["-e", "-o", "pid=,ppid="])
        .output()
        .context("read the process table for nested build descendants")?;
    ensure!(
        output.status.success(),
        "reading the process table failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some((parent, child)) = parent_child(line) {
            children.entry(parent).or_default().push(child);
        }
    }
    let mut pids = Vec::new();
    let mut frontier = vec![root];
    while let Some(parent) = frontier.pop() {
        for child in children.get(&parent).into_iter().flatten() {
            if !pids.contains(child) {
                pids.push(*child);
                frontier.push(*child);
            }
        }
    }
    Ok(Descendants { pids })
}

/// Parse one `pid ppid` row of the process table.
#[cfg(unix)]
fn parent_child(line: &str) -> Option<(u32, u32)> {
    let mut fields = line.split_whitespace();
    let pid = fields.next()?.parse().ok()?;
    let parent = fields.next()?.parse().ok()?;
    Some((parent, pid))
}

/// Kill `raw` and, when it leads one, its whole process group.
///
/// Returns the signal error so the caller can explain a survivor. `ESRCH` for
/// a descendant that has already exited is expected and is not reported on its
/// own: the caller only renders these failures beside a pid that is still
/// alive.
#[cfg(unix)]
fn terminate_descendant(raw: u32) -> Option<Errno> {
    let Ok(raw_pid) = i32::try_from(raw) else {
        return None;
    };
    let pid = Pid::from_raw(raw_pid);
    let signal = match getpgid(Some(pid)) {
        Ok(group) if group == pid => killpg(pid, Signal::SIGKILL),
        _ => kill(pid, Signal::SIGKILL),
    };
    signal.err()
}

/// Render the signals a recorded descendant refused, for the survivor message.
#[cfg(unix)]
fn describe_signal_failures(failures: &[(u32, Errno)]) -> String {
    let rendered = failures
        .iter()
        .map(|(pid, error)| format!("{pid}: {error}"))
        .collect::<Vec<_>>()
        .join(", ");
    if rendered.is_empty() {
        return String::new();
    }
    format!(" (signal failures: {rendered})")
}

/// Whether `raw` is still running.
///
/// A killed descendant whose parent has not reaped it yet is a zombie: it
/// cannot refuse `SIGKILL` and cannot write to the target, but `kill(pid, 0)`
/// still reports it as present. Treating only a live process as alive keeps the
/// bounded wait from reporting a descendant that has already stopped.
#[cfg(unix)]
fn is_alive(raw: u32) -> bool {
    let Ok(raw_pid) = i32::try_from(raw) else {
        return false;
    };
    match kill(Pid::from_raw(raw_pid), None) {
        Ok(()) => !is_dead_state(process_state(raw_pid)),
        Err(Errno::ESRCH) => false,
        // A process this one may not signal is still running.
        Err(_) => true,
    }
}

/// The process-table state for `raw`, when the table can be read.
#[cfg(unix)]
fn process_state(raw: i32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "stat=", "-p"])
        .arg(raw.to_string())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Whether a recorded state is a process that has already stopped.
///
/// An unreadable state is not treated as stopped, so the caller keeps waiting
/// rather than claiming a termination it could not observe.
#[cfg(unix)]
fn is_dead_state(state: Option<String>) -> bool {
    state.is_some_and(|text| text.starts_with('Z') || text.starts_with('X'))
}

/// Record the descendants of `root`.
///
/// Windows records none: the helper's Job Object terminates its own tree when
/// the helper dies, so the parent has nothing left to remove.
///
/// # Errors
///
/// This implementation cannot fail.
#[cfg(not(unix))]
pub(crate) const fn snapshot_descendants(_root: u32) -> Result<Descendants> {
    Ok(Descendants {})
}

#[cfg(all(test, unix))]
mod tests {
    //! The process-table parse and the snapshot's tree walk.

    use super::{Descendants, parent_child};
    use anyhow::{Context, Result, ensure};
    use std::process::Command;

    #[test]
    fn a_process_row_yields_its_parent_and_pid() {
        assert_eq!(parent_child("  123  45"), Some((45, 123)));
        assert_eq!(parent_child("1 0"), Some((0, 1)));
        assert_eq!(parent_child("garbage"), None);
    }

    #[test]
    fn the_snapshot_finds_a_live_child() -> Result<()> {
        let mut child = Command::new("sleep")
            .arg("30")
            .spawn()
            .context("start a descendant to record")?;
        let snapshot = super::snapshot_descendants(std::process::id())?;
        ensure!(
            snapshot.pids.contains(&child.id()),
            "the snapshot should record the direct child",
        );
        let _kill = child.kill();
        let _wait = child.wait();
        Ok(())
    }

    #[test]
    fn termination_removes_a_recorded_child() -> Result<()> {
        let mut child = Command::new("sleep")
            .arg("30")
            .spawn()
            .context("start a descendant to terminate")?;
        let snapshot = Descendants {
            pids: vec![child.id()],
        };
        snapshot.terminate()?;
        let status = child.wait().context("reap the terminated descendant")?;
        ensure!(
            !status.success(),
            "the descendant should have been killed, not exited normally",
        );
        Ok(())
    }
}
