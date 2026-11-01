//! Naming a process from `/proc`, for a lock-holder report.
//!
//! A holder report is only actionable if it says *which* process holds the lock
//! and how it relates to the reader. Both come from `/proc/<pid>/stat`, which
//! is a racing snapshot like the lock table itself: a process can exit between
//! the record naming it and the read that would describe it, so every lookup
//! degrades to a weaker sentence rather than failing the report.
//!
//! This lives apart from `artefact_lock_diagnostics` so that each file stays
//! within the length cap, and because reading `/proc/<pid>/stat` is a distinct
//! concern from deciding what a hold means. Everything here is Linux-only.

// Gated with every item below: on any other target this module is empty, and
// an ungated import would then be an unused one under `-D warnings`.
#[cfg(target_os = "linux")]
use super::lock_evidence::ProcessId;
#[cfg(target_os = "linux")]
use std::fs;

/// Names a process by pid, command, parent, and relation to the reporting one.
#[cfg(target_os = "linux")]
pub(super) fn describe_process(pid: ProcessId, own_pid: ProcessId) -> String {
    match process_identity(pid) {
        Some(identity) => format!(
            "pid {pid} ({}), parent {}, {}",
            identity.command,
            identity.parent,
            relation(pid, Some(identity.parent), own_pid)
        ),
        None => format!(
            "pid {pid} (<stat unreadable>), {}",
            relation(pid, None, own_pid)
        ),
    }
}

/// Labels a pid by how it relates to the process building the report.
#[cfg(target_os = "linux")]
fn relation(pid: ProcessId, parent: Option<ProcessId>, own_pid: ProcessId) -> &'static str {
    if pid == own_pid {
        return "this process";
    }
    match parent {
        Some(reported) if reported == own_pid => "child of this process",
        Some(_) => "other",
        None => "other (parent unknown)",
    }
}

/// What `/proc/<pid>/stat` says about one process.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub(super) struct ProcessIdentity {
    /// The command name the kernel records, truncated to 15 bytes by `comm`.
    pub(super) command: String,
    /// The pid of the process that forked this one.
    pub(super) parent: ProcessId,
}

/// Reads a process's command name and parent pid from `/proc/<pid>/stat`.
///
/// The command name sits in parentheses and may itself contain spaces or
/// parentheses, so the fields after it are located from the *last* closing
/// parenthesis rather than by splitting the whole line: splitting would make a
/// process called `a) b` look like a different process entirely.
#[cfg(target_os = "linux")]
pub(super) fn process_identity(pid: ProcessId) -> Option<ProcessIdentity> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let command = stat.get(open.saturating_add(1)..close)?.to_owned();
    let mut trailing = stat.get(close.saturating_add(1)..)?.split_whitespace();
    // Past the command name the first field is the state and the second the
    // parent pid.
    let _state = trailing.next()?;
    let parent = ProcessId::new(trailing.next()?.parse().ok()?);
    Some(ProcessIdentity { command, parent })
}
