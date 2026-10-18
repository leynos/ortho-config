//! A fixed documentation-scenario envelope inside nextest's per-test ceiling.

use crate::process_runner::Policy;
use anyhow::{Context, Result};
use std::time::{Duration, Instant};

// The workflow contract pins this to the actual nextest configuration. Reserve
// two cleanup allowances: child cleanup and time for the test to report it.
pub(super) const PER_TEST_SECONDS: u64 = 600;

/// Start one immutable wall-clock envelope; output never extends it.
pub(super) fn deadline() -> Instant {
    Instant::now()
        + Duration::from_secs(PER_TEST_SECONDS)
            .saturating_sub(Policy::build().cleanup_timeout().saturating_mul(2))
}

/// Report exhaustion before starting another operation in the same scenario.
pub(super) fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .context("documentation scenario exhausted its overall deadline")
}
