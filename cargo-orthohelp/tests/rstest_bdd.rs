//! Integration test entry point for `cargo-orthohelp` behavioural scenarios.
//!
//! The [`rstest_bdd_support`] module binds Gherkin feature files to step
//! definitions for CLI invocation, cache success and failure cases, and IR,
//! roff, and `PowerShell` output contracts. The [`fixtures`] module provides
//! shared test helpers, including binary-path discovery for the generated
//! command.

mod fixtures;
mod rstest_bdd_support;
