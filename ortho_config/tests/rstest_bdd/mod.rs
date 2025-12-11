//! `rstest-bdd` scaffolding for `ortho_config`.
//!
//! The modules defined alongside this entrypoint register reusable fixtures,
//! the primary behavioural steps, and a canary scenario so that the
//! `rstest-bdd` macros execute under `cargo test` without needing to disable
//! the harness.

#[path = "../common/mod.rs"]
pub mod common;

mod behaviour;
mod canary;
mod canary_steps;
mod fixtures;
