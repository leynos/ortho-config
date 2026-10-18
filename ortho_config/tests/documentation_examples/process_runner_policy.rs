//! Operation-specific deadlines for documentation-test subprocesses.

use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Build,
    Execution,
    Discovery,
}

impl Kind {
    const fn name(self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Execution => "execution",
            Self::Discovery => "discovery",
        }
    }
}

/// Bounds one documentation-test operation and its cleanup.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Policy {
    kind: Kind,
    timeout: Duration,
    cleanup_timeout: Duration,
    output_limit: usize,
}

impl Policy {
    /// Retain the existing ten-minute ceiling until measured build timing can replace it.
    pub(crate) const fn build() -> Self {
        Self::new(Kind::Build, Duration::from_secs(600))
    }

    /// Bound execution independently from compilation and discovery.
    pub(crate) const fn execution() -> Self {
        Self::new(Kind::Execution, Duration::from_secs(60))
    }

    /// Bound executable discovery independently from compilation and execution.
    pub(crate) const fn discovery() -> Self {
        Self::new(Kind::Discovery, Duration::from_secs(30))
    }

    const fn new(kind: Kind, timeout: Duration) -> Self {
        Self {
            kind,
            timeout,
            cleanup_timeout: Duration::from_secs(2),
            output_limit: 1024 * 1024,
        }
    }

    /// Override the operation deadline for a controlled probe.
    pub(crate) const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Override the bounded cleanup allowance for a controlled probe.
    pub(crate) const fn with_cleanup_timeout(mut self, timeout: Duration) -> Self {
        self.cleanup_timeout = timeout;
        self
    }

    /// Override per-stream capture for a controlled probe.
    pub(crate) const fn with_output_limit(mut self, output_limit: usize) -> Self {
        self.output_limit = output_limit;
        self
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
        self.kind.name()
    }
}
