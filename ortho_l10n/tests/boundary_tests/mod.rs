//! Integration-test entry point for the standalone-boundary contracts.
//!
//! Declared as `[[test]] name = "boundary"` with `path =
//! "tests/boundary_tests/mod.rs"` in `ortho_l10n/Cargo.toml`. The directory
//! mount keeps the support modules (`tests/boundary/`) out of Cargo's target
//! discovery: only this file is a target, so adding a support module never
//! adds a binary that Cargo then has to build and nextest has to budget.

#[path = "../boundary/mod.rs"]
mod boundary;

mod closure;
mod consumer;
mod manifests;
