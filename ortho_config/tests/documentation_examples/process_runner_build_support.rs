//! Runtime process supervision for Cargo build integration tests.
//!
//! The identifier-artefact fixture is the one caller that hands a nested child
//! an inherited standard input: a duplicate of the target lease, so an
//! interrupted build keeps the target leased through its compilers. Those two
//! entry points live here rather than beside the shared runner so the other
//! test binaries, which never need them, do not carry unused code.

#[path = "process_runner.rs"]
mod implementation;

use anyhow::Result;
use implementation::{RunHooks, kill_child, poll_child, run_with_hooks_stdin};
use std::process::{Command, Output, Stdio};

pub(super) use implementation::{Operation, Policy};
pub(crate) use implementation::{cleanup_incomplete, deadline_exceeded, run_with_policy};

/// Run a command whose child must inherit `stdin`.
///
/// This is [`run_with_policy`] with the standard input the caller supplies.
/// Every other caller gets the silent default, because a subprocess that reads
/// an inherited terminal would block the test.
pub(crate) fn run_with_policy_with_stdin(
    command: &mut Command,
    operation: Operation<'_>,
    policy: Policy,
    stdin: Stdio,
) -> Result<Output> {
    run_with_observer_with_stdin(command, operation, policy, stdin, |_| {})
}

/// Observe a command's output while its child inherits `stdin`.
///
/// This is [`run_with_observer`] with the standard input the caller supplies,
/// used by the fixture build for the same reason as
/// [`run_with_policy_with_stdin`].
pub(crate) fn run_with_observer_with_stdin<Observer>(
    command: &mut Command,
    operation: Operation<'_>,
    policy: Policy,
    stdin: Stdio,
    observer: Observer,
) -> Result<Output>
where
    Observer: FnMut(&[u8]) + Send + 'static,
{
    run_with_hooks_stdin(
        command,
        operation,
        policy,
        Some(stdin),
        RunHooks {
            observer,
            poll: poll_child,
            kill: kill_child,
        },
    )
}
