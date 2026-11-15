//! `rstest-bdd` scaffolding for the `hello_world` example crate.
//!
//! The modules provide fixtures, step registrations, the migrated behavioural
//! suite, and a canary scenario so coverage runs under the stock `cargo test`
//! harness.

mod behaviour;
mod canary;
mod fixtures;
mod steps;
