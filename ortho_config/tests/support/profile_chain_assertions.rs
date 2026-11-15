//! Assertions shared by the `extends` suite and its split-out profile cases.
//!
//! Split from `extends.rs` so each suite stays within the repository's
//! 400-line code file ceiling, and so the labelled equality assertion has one
//! definition rather than one per suite.

use anyhow::{Result, ensure};

/// Assert two values are equal, naming the field on failure.
///
/// # Errors
///
/// Returns an error when the values differ, printing both the expected and the
/// actual value alongside the label.
pub fn ensure_eq<T>(actual: &T, expected: &T, label: &str) -> Result<()>
where
    T: PartialEq + std::fmt::Debug,
{
    ensure!(
        actual == expected,
        "unexpected {label} {actual:?}; expected {expected:?}"
    );
    Ok(())
}
