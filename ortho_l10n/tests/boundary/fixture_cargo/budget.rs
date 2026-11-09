//! A per-contract allowance shared by every nested Cargo call in one test.
//!
//! nextest bounds each test as a whole, but a per-invocation deadline cannot
//! see that whole. A contract that makes several calls can therefore spend the
//! test's allowance on the earlier ones and be stopped by nextest before a
//! later call reports its own deadline; the failure then names nextest's
//! timeout rather than the invocation that hung. [`Budget`] closes that gap:
//! every call is bounded by the smaller of its own cap and what the contract
//! has left, and a call that starts with nothing left is reported as an
//! exhausted allowance naming itself.
//!
//! The budget is passed explicitly rather than kept in a thread-local. `cargo
//! test --test-threads=1` runs every test on one thread, so a thread-local
//! would silently accumulate the calls of unrelated contracts.

use anyhow::{Result, ensure};
use std::time::{Duration, Instant};

/// Longest the nested Cargo calls in one boundary contract may take together.
///
/// The boundary override grants 960 s per test. Sixty seconds stay outside the
/// budget for rendering fixture roots, reading manifests, and reporting, so the
/// calls between them may use 900 s. The budget is smaller than the allowance
/// for the same reason each per-call cap is: a bound that cannot be reached
/// before nextest stops the test reports nothing about the invocation that
/// hung.
pub(crate) const SCENARIO_ALLOWANCE: Duration = Duration::from_secs(900);

/// A deadline shared by the nested Cargo calls in one contract.
///
/// Copy rather than borrowed, so a fixture can hold one without a lifetime
/// parameter on every type that reaches Cargo.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Budget {
    /// Instant at which the contract's shared allowance is spent.
    deadline: Instant,
}

impl Budget {
    /// Start a contract's shared allowance now.
    pub(crate) fn new() -> Self {
        Self::expiring_in(SCENARIO_ALLOWANCE)
    }

    /// Start an allowance of `allowance`, for regressions that cannot wait.
    #[cfg(test)]
    fn expiring_in(allowance: Duration) -> Self {
        Self {
            deadline: Instant::now() + allowance,
        }
    }

    /// Bound one nested invocation by its own cap or the allowance, whichever is sooner.
    ///
    /// A call that starts after the allowance is spent is refused here rather
    /// than spawned with a zero deadline, so the error names the invocation
    /// that never ran and says why it could not.
    pub(crate) fn bound(&self, cap: Duration, description: &str) -> Result<Bound> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        ensure!(
            left > Duration::ZERO,
            "the boundary contract's {} second allowance was exhausted before \
             {description} could start; an earlier nested Cargo invocation used it",
            SCENARIO_ALLOWANCE.as_secs(),
        );
        let limited_by_allowance = left < cap;
        Ok(Bound {
            deadline: if limited_by_allowance { left } else { cap },
            limited_by_allowance,
        })
    }
}

/// The bound for one nested invocation, and which limit produced it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bound {
    /// How long the invocation may run.
    pub(crate) deadline: Duration,
    /// Whether the shared allowance, rather than the call's own cap, set it.
    pub(crate) limited_by_allowance: bool,
}

#[cfg(test)]
mod tests {
    //! Which limit sets a call's bound, and what an exhausted allowance says.

    use super::{Budget, SCENARIO_ALLOWANCE};
    use anyhow::{Result, ensure};
    use std::time::Duration;

    /// A cap that fits inside the allowance is used unchanged.
    #[test]
    fn a_cap_shorter_than_the_allowance_is_used_as_is() -> Result<()> {
        let bound = Budget::new().bound(Duration::from_secs(5), "a probe")?;
        ensure!(
            bound.deadline == Duration::from_secs(5),
            "the call's own cap should be used unchanged, got {:?}",
            bound.deadline,
        );
        ensure!(
            !bound.limited_by_allowance,
            "a cap inside the allowance is not limited by it",
        );
        Ok(())
    }

    /// A cap longer than what is left is clamped, and says so.
    #[test]
    fn a_short_allowance_clamps_the_cap() -> Result<()> {
        let bound =
            Budget::expiring_in(SCENARIO_ALLOWANCE / 2).bound(SCENARIO_ALLOWANCE, "a probe")?;
        ensure!(
            bound.deadline <= SCENARIO_ALLOWANCE / 2,
            "the remaining allowance should clamp the cap, got {:?}",
            bound.deadline,
        );
        ensure!(
            bound.deadline > Duration::ZERO,
            "a clamped bound should still be usable, got {:?}",
            bound.deadline,
        );
        ensure!(
            bound.limited_by_allowance,
            "a clamped bound should report the allowance as its limit",
        );
        Ok(())
    }

    /// A spent allowance refuses the call and names the invocation.
    #[test]
    fn an_exhausted_allowance_names_the_invocation() {
        let error = Budget::expiring_in(Duration::ZERO)
            .bound(Duration::from_secs(5), "cargo metadata")
            .expect_err("a spent allowance cannot bound an invocation");
        assert!(
            error.to_string().contains("cargo metadata"),
            "the error should name the invocation that could not start: {error}"
        );
    }
}
