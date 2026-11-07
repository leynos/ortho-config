//! Deadline helpers for documentation scenarios with explicit nextest ceilings.

use crate::process_runner::Policy;
use anyhow::{Context, Result};
use std::time::{Duration, Instant};

// The workflow contract pins this to the actual nextest configuration. Reserve
// two cleanup allowances: child cleanup and time for the test to report it.
pub(super) const PER_TEST_SECONDS: u64 = 960;

/// Start one immutable wall-clock envelope tied to the test binary's allowance.
///
/// Pass `None` to use `PER_TEST_SECONDS`. A larger explicit allowance is for a
/// test whose binary has a matching nextest override and workflow contract.
/// Every subprocess receives only the remaining time in this envelope.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// let cleanup = Duration::from_secs(2);
/// let configured_seconds = Some(1_080).unwrap_or(960);
/// let allowance = Duration::from_secs(configured_seconds);
/// let envelope = allowance.saturating_sub(cleanup.saturating_mul(2));
/// assert_eq!(envelope, Duration::from_secs(1_076));
/// ```
pub(super) fn deadline_for(allowance_seconds: Option<u64>) -> Instant {
    let selected_allowance_seconds = allowance_seconds.unwrap_or(PER_TEST_SECONDS);
    Instant::now()
        + Duration::from_secs(selected_allowance_seconds)
            .saturating_sub(Policy::build().cleanup_timeout().saturating_mul(2))
}

/// Report exhaustion before starting another operation in the same scenario.
pub(super) fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .context("documentation scenario exhausted its overall deadline")
}
