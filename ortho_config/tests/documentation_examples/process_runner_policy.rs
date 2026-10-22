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
        Self::new(
            "build",
            Duration::from_secs(956),
            Duration::from_secs(2),
            1024 * 1024,
        )
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
