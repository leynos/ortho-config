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
use super::lock_procfs::ProcFs;

/// Names a process by pid, command, parent, and relation to the reporting one.
///
/// Reads the stat line through `procfs` rather than the filesystem, so a test
/// can describe a process the kernel has never heard of, and can present an
/// unreadable stat line without needing a pid that is guaranteed to be missing.
///
/// The unreadable case is written as an explicit sentence rather than folded
/// into "no parent". A reader deciding whether a child inherited the lock needs
/// to know that the relation is *unknown*, which is a different fact from the
/// process having no parent recorded.
#[cfg(target_os = "linux")]
pub(super) fn describe_process(pid: ProcessId, own_pid: ProcessId, procfs: &dyn ProcFs) -> String {
    procfs.process_stat(pid).map_or_else(
        || {
            format!(
                "pid {pid} (<stat unreadable>), {}",
                relation(pid, None, own_pid)
            )
        },
        |identity| {
            format!(
                "pid {pid} ({}), parent {}, {}",
                identity.command(),
                identity.parent(),
                relation(pid, Some(identity.parent()), own_pid)
            )
        },
    )
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

// The stat line's parsing lives with the read that supplies it, in
// `lock_procfs`, so this module names a process and that one reads it.
