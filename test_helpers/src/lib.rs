//! Test helpers shared across crates.
//!
//! This crate provides guards for process-global state such as environment
//! variables and the working directory.
//!
//! Usage scope:
//! - Intended for test code only; do not use in production binaries or libraries.

pub mod cwd;
pub mod env;
pub mod figment;
pub mod text;
