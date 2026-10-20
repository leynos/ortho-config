//! Process-runner regressions owned by the infrastructure test target.

#[path = "process_runner.rs"]
mod implementation;

#[cfg(target_os = "linux")]
pub(super) use implementation::tree;
pub(super) use implementation::wait::{WaitProcess, wait_for_exit};
pub(super) use implementation::{Operation, Policy};
pub(super) use implementation::{
    RunHooks, capture, finish_wrapped_spawn, run_with_hooks, spawn_guard, spawn_owned,
};
pub(crate) use implementation::{
    cleanup_incomplete, deadline_exceeded, run_with_observer, run_with_policy,
};

#[path = "process_runner_tests.rs"]
mod tests;
