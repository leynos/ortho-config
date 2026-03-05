//! Local test helpers for `ortho_config` tests.
//!
//! These modules mirror the shared workspace helpers so this crate can be
//! published independently without a non-publishable dev-dependency.
#![allow(
    dead_code,
    reason = "Each test target consumes a subset of shared helper APIs."
)]

pub mod cwd;
pub mod env;
pub mod figment;
pub mod text;
