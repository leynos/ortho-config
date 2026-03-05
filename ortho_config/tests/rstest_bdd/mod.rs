//! `rstest-bdd` scaffolding for `ortho_config`.
//!
//! The modules defined alongside this entrypoint register reusable fixtures,
//! the primary behavioural steps, and a canary scenario so that the
//! `rstest-bdd` macros execute under `cargo test` without needing to disable
//! the harness.

/// Shared test fixtures for integration tests.
#[path = "../fixtures/mod.rs"]
pub mod fixtures;

#[path = "../support/default_punct.rs"]
mod default_punct;

mod behaviour;
mod canary;
mod canary_steps;
mod scenario_state;
