//! Shared helpers for unit and integration tests within the `hello_world` crate.
//! This module simply re-exports the workspace-wide figment utilities so tests
//! can depend on a single implementation.

pub use test_helpers::figment::{figment_error, with_jail};
