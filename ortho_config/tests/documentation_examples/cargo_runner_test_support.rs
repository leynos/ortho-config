//! Cargo-runner regressions owned by the infrastructure test target.

#[path = "cargo_runner.rs"]
mod implementation;

pub(super) use implementation::cargo_command;
pub(super) use implementation::cargo_environment;
pub(super) use implementation::cargo_environment::CARGO_ENV_ALLOWLIST;
#[cfg(all(windows, target_env = "msvc", target_arch = "x86_64"))]
use implementation::{allowed_environment, first_output_line};
pub(super) use implementation::{
    cargo_dispatch_path, prepare_cargo_command_for_target_before, prepare_metadata_command,
};

#[path = "cargo_environment_tests.rs"]
mod cargo_environment_tests;
#[path = "cargo_runner_tests.rs"]
mod tests;
