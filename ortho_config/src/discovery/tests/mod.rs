//! Integration tests for configuration discovery across multiple platforms.
//!
//! Verifies candidate ordering, XDG/Windows/HOME directory resolution, project roots,
//! environment variable overrides, and error propagation for required/optional paths.

mod candidates;
mod fixtures;
mod loading;
mod platform;
