//! Operation-specific deadlines for documentation-test subprocesses.

use std::time::Duration;

/// Bounds one documentation-test operation and its cleanup.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Policy {
    kind: &'static str,
    timeout: Duration,
    cleanup_timeout: Duration,
    output_limit: usize,
}

impl Policy {
    /// Fit inside the fixed scenario envelope after its cleanup reservations.
    pub(crate) const fn build() -> Self {
        Self::build_with_timeout(Duration::from_secs(956))
    }

    /// Create a build policy with a caller-selected operation cap.
    ///
    /// Use this only when the test binary has a matching nextest override;
    /// callers must still bound the policy by their scenario's remaining time.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let allowance = Duration::from_secs(1_080);
    /// let cleanup = Duration::from_secs(2);
    /// let envelope = allowance.saturating_sub(cleanup.saturating_mul(2));
    /// assert_eq!(envelope, Duration::from_secs(1_076));
    /// ```
    pub(crate) const fn build_with_timeout(timeout: Duration) -> Self {
        Self::new("build", timeout, Duration::from_secs(2), 1024 * 1024)
    }

    /// Bound executable discovery independently from compilation and execution.
    pub(crate) const fn discovery() -> Self {
        Self::new(
            "discovery",
            Duration::from_secs(30),
            Duration::from_secs(2),
            1024 * 1024,
        )
    }

    /// Select an operation's hard deadline, cleanup allowance and capture bound.
    pub(crate) const fn new(
        kind: &'static str,
        timeout: Duration,
        cleanup_timeout: Duration,
        output_limit: usize,
    ) -> Self {
        Self {
            kind,
            timeout,
            cleanup_timeout,
            output_limit,
        }
    }

    /// Cap only the operation deadline while preserving cleanup and capture policy.
    pub(crate) fn bounded_by(self, remaining: Duration) -> Self {
        Self {
            timeout: self.timeout.min(remaining),
            ..self
        }
    }

    pub(crate) const fn timeout(self) -> Duration {
        self.timeout
    }

    pub(crate) const fn cleanup_timeout(self) -> Duration {
        self.cleanup_timeout
    }

    pub(crate) const fn output_limit(self) -> usize {
        self.output_limit
    }

    pub(crate) const fn kind_name(self) -> &'static str {
        self.kind
    }
}
