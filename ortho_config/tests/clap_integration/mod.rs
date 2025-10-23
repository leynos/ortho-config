//! Integration tests for CLI parsing across multiple configuration sources.
//!
//! The submodules are grouped by feature area to keep each file concise and to
//! encourage parameterised coverage via `rstest` cases.

mod common;
mod parsing;
mod error_cases;
mod option_cases;
mod config_path;

#[cfg(any(unix, target_os = "redox"))]
mod xdg;
