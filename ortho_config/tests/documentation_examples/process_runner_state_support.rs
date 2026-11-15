//! Runtime process supervision for CLI tests with ownership recovery checks.

#[path = "process_runner.rs"]
mod implementation;

pub(super) use implementation::{Operation, Policy};
pub(crate) use implementation::{cleanup_incomplete, deadline_exceeded, run_with_policy};
