//! Runtime process supervision for Cargo build integration tests.
//!
//! The identifier-artefact fixture is the one caller that hands a nested child
//! an inherited standard input: a duplicate of the target lease, so an
//! interrupted build keeps the target leased through its compilers. That entry
//! point lives here rather than beside the shared runner so the other test
//! binaries, which never need it, do not carry unused code.
//!
//! The hooks themselves are re-exported because the fixture build observes
//! Cargo's message stream while inheriting the same handle; constructing its
//! own [`RunHooks`] keeps the observed and silent paths on one runner.

#[path = "process_runner.rs"]
mod implementation;

use anyhow::Result;
use std::process::{Command, Output, Stdio};

pub(super) use implementation::{
    Operation, Policy, RunHooks, kill_child, poll_child, run_with_hooks,
};
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
    run_with_hooks(
        command,
        operation,
        policy,
        RunHooks {
            stdin: Some(stdin),
            observer: |_: &[u8]| {},
            poll: poll_child,
            kill: kill_child,
        },
    )
}
