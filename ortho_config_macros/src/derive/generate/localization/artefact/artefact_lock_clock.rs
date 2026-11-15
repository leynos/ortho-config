//! The clock and sleeper the artefact-lock settle reads through.
//!
//! [`ensure_settles_free_observed`] makes two kinds of temporal decision: when
//! its deadline has passed, and how long to wait between probes. Calling
//! `Instant::now` and `thread::sleep` directly made both decisions untestable
//! in the only cases that matter — a lock that outlives the whole budget, and a
//! probe sequence that must sleep *between* probes and not after the last one.
//! Reaching those with a real clock means waiting out the real budget, so the
//! boundary test could only be an expensive one that still raced the machine.
//!
//! [`Clock`] is that boundary. [`RealClock`] delegates to the standard library
//! and is what production supplies; a test supplies a fake that advances only
//! when asked, which turns "does the settle return the hold report at the
//! deadline" into an assertion about the code rather than a race with it.
//!
//! [`ensure_settles_free_observed`]: super::lock_diagnostics::ensure_settles_free_observed

use std::time::{Duration, Instant};

/// The clock and sleeper a settle uses to bound its wait.
///
/// `now` and `sleep` are one trait rather than two because they are only ever
/// used together: a settle that asked one clock for the time and a different
/// seam to sleep would not be able to reason about its own deadline. Keeping
/// them paired means a fake clock that advances on `sleep` models the passage
/// of time the settle actually depends on.
pub(super) trait Clock {
    /// Returns the current instant.
    fn now(&self) -> Instant;

    /// Waits for roughly `duration`.
    ///
    /// Roughly, not exactly: this bounds a retry interval, and no assertion
    /// depends on the wait being precise.
    fn sleep(&self, duration: Duration);
}

/// The standard-library clock: what production always uses.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct RealClock;

impl Clock for RealClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
