//! Runtime Cargo-runner support for executable examples.

#[path = "cargo_runner.rs"]
mod implementation;

pub(super) use implementation::{
    cargo_dispatch_path, prepare_cargo_command_for_target_before, prepare_metadata_command,
};
