//! Deadline resolution for the subprocess runner.
//!
//! The runner file owns spawning, capture and cleanup; deciding *when* a child
//! must finish is a small, pure calculation with its own tests, so it lives
//! here. Keeping it apart also keeps `runner.rs` inside the file-length cap.

use std::time::{Duration, Instant};

/// The scope label for a budget the command itself owns.
pub(super) const COMMAND_SCOPE: &str = "command";
/// The scope label for the remainder of an enclosing sequence's budget.
pub(super) const SEQUENCE_SCOPE: &str = "shared sequence (remaining budget)";

/// The operation deadline and labels used in timeout diagnostics.
#[derive(Clone, Copy)]
pub(super) struct CommandDeadline<'a> {
    /// Monotonic instant by which the command must exit.
    pub(super) at: Instant,
    /// Configured operation budget, for the diagnostic message.
    pub(super) duration: Duration,
    /// Whether the budget came from the command or its enclosing sequence.
    pub(super) scope: &'a str,
    /// Human-readable command description for error messages.
    pub(super) description: &'a str,
}

/// Resolve the deadline a command runs under, measured from *now*.
///
/// The earlier of the command's own budget and the enclosing sequence's
/// remainder wins. The instant is derived from the caller's `now` rather than
/// from the moment the child has spawned: spawning and starting the readers
/// take real time, and charging that to nobody would let a sequence deadline
/// with a fraction of a second left hand the child its whole command budget
/// afterwards. `None` means the sum overflowed the clock.
pub(super) fn effective_deadline(
    command_deadline: Duration,
    sequence_deadline: Option<Instant>,
    now: Instant,
    description: &str,
) -> Option<CommandDeadline<'_>> {
    let command_at = now.checked_add(command_deadline)?;
    match sequence_deadline {
        Some(sequence) if sequence < command_at => Some(CommandDeadline {
            at: sequence,
            duration: sequence.saturating_duration_since(now),
            scope: SEQUENCE_SCOPE,
            description,
        }),
        _ => Some(CommandDeadline {
            at: command_at,
            duration: command_deadline,
            scope: COMMAND_SCOPE,
            description,
        }),
    }
}
