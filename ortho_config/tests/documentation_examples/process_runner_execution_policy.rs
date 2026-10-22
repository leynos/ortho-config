//! Execution budget for documented binaries and BDD command runs.
//!
//! Keep this caller policy in test roots that execute documented applications;
//! Cargo build callers use the separate build policy from the process runner.

use crate::process_runner::Policy;
use std::time::Duration;

/// Bound one documented application execution independently from build work.
pub(crate) const fn policy() -> Policy {
    Policy::new(
        "execution",
        Duration::from_secs(60),
        Duration::from_secs(2),
        1024 * 1024,
    )
}
