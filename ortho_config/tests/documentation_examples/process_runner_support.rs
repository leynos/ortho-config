//! Runtime process-runner support for executable examples.

#[path = "process_runner.rs"]
mod implementation;

pub(super) use implementation::{Operation, Policy};
pub(crate) use implementation::{cleanup_incomplete, deadline_exceeded, run_with_policy};
