//! Integration test entry point for `cargo-orthohelp` behavioural scenarios.
//!
//! The [`rstest_bdd_support`] module binds Gherkin feature files to step
//! definitions for CLI invocation, cache success and failure cases, and IR,
//! roff, and `PowerShell` output contracts. The [`fixtures`] module provides
//! shared test helpers, including binary-path discovery for the generated
//! command.

#[path = "../../ortho_config/tests/documentation_examples/cargo_environment.rs"]
mod cargo_environment;
#[path = "../../ortho_config/tests/documentation_examples/process_runner_support.rs"]
mod process_runner;

mod fixtures;
mod rstest_bdd_support;
