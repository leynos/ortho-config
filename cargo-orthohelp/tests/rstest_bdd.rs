//! Integration test entry point for `cargo-orthohelp` behavioural scenarios.
//!
//! The [`rstest_bdd`] module binds Gherkin feature files to step definitions
//! for CLI invocation, cache success and failure cases, and IR, roff, and
//! `PowerShell` output contracts. The [`fixtures`] module provides shared test
//! helpers, including binary-path discovery for the generated command.

#[path = "fixtures/mod.rs"]
mod fixtures;

#[path = "rstest_bdd/mod.rs"]
mod rstest_bdd;
