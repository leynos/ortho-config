//! `rstest-bdd` scaffolding for `ortho_config`.
//!
//! The modules defined alongside this entrypoint register reusable fixtures,
//! step implementations, and a canary scenario so that the `rstest-bdd`
//! macros execute under `cargo test` without needing to disable the harness.

mod fixtures;
mod steps;
mod canary;
