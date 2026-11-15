//! Deadline-bounded subprocess support for workspace tests.
//!
//! The module owns a spawned command's process tree and its captured output
//! readers. Callers choose command arguments, environment, and the operation
//! deadline; they should use this module rather than supervising descendants
//! or pipe-reader threads themselves.

#[path = "capture.rs"]
#[doc(hidden)]
pub mod capture;
#[cfg(unix)]
#[path = "pipe_unix.rs"]
#[doc(hidden)]
pub mod pipe;
#[cfg(windows)]
#[path = "pipe_windows.rs"]
#[doc(hidden)]
pub mod pipe;
#[cfg(any(unix, windows))]
#[path = "spawn_guard.rs"]
#[doc(hidden)]
pub mod spawn_guard;
#[cfg(unix)]
#[path = "tree_unix.rs"]
#[doc(hidden)]
pub mod tree;
#[cfg(windows)]
#[path = "tree_windows.rs"]
#[doc(hidden)]
pub mod tree;

mod runner;

pub use runner::{run_with_deadline, run_with_deadline_until};
