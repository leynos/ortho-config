"""Reading the timers that can end a test run.

The contract in ``timeout_ordering_test`` compares budgets written down
in three different files. Turning those files into comparable seconds is
the part that can be wrong without any file being wrong, so it lives
here where it can be read on its own.

See "Test timeouts: four tiers, outermost last" in
``docs/developers-guide.md``.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

REPO_ROOT: typ.Final[Path] = Path(__file__).resolve().parents[2]
WORKFLOWS_DIRECTORY: typ.Final[Path] = REPO_ROOT / ".github" / "workflows"
NEXTEST_CONFIG: typ.Final[Path] = REPO_ROOT / ".config" / "nextest.toml"

#: The environment variable the shared coverage action reads for its
#: wall-clock cap on one `cargo` invocation.
WATCHDOG_VARIABLE: typ.Final[str] = "RUN_RUST_CARGO_WAIT_TIMEOUT"

#: The action whose steps run under that watchdog.
COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage"
)

#: Everything in a coverage job that is not a `cargo` invocation the
#: watchdog bounds: checkout, toolchain setup, and above all the cache
#: save and restore. The job timer covers it; the watchdog does not.
#:
#: Per workflow, because the two lanes differ by an order of magnitude
#: and holding the trunk lane to the pull-request lane's figure would
#: demand a ceiling its own runs cannot justify.
#:
#: Measured from the worst of many runs rather than one, and across runs
#: of every conclusion rather than successful ones only, since a run
#: cancelled at its ceiling is the case the sizing exists to prevent.
#: The gap is the job's duration less its two watchdog-bounded coverage
#: steps, so it is exactly the work the job timer covers and the
#: watchdogs do not.
#:
#: - `ci.yml`: 3,257 s on the Windows leg of run 33447440225, whose two
#:   coverage steps took 1,323 s and 950 s of a 5,530 s job. Read across
#:   103 jobs, 100 successful and the rest failed or cancelled. Allowed
#:   60 minutes.
#: - `coverage-main.yml`: 284 s on run 31908409573, read across 31 runs,
#:   29 successful and 2 failed. Allowed 15 minutes.
#:
#: No run in either sample was ended by any of these four timers: the
#: worst `ci.yml` job reached 5,530 s of its ceiling. None of them was
#: genuinely cold either.
OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS: typ.Final[dict[str, float]] = {
    "ci.yml": 60 * 60.0,
    "coverage-main.yml": 15 * 60.0,
}

#: What an unmeasured workflow is held to. The larger of the two above,
#: so a new coverage lane meets the stricter requirement until someone
#: measures it and adds its own figure with a run id.
DEFAULT_OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS: typ.Final[float] = 60 * 60.0

CEILING_MARGIN_SECONDS: typ.Final[float] = 15 * 60.0

#: What nextest allows a test between `SIGTERM` and `SIGKILL` when the
#: configuration names no `grace-period`, as this one does not.
NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS: typ.Final[float] = 10.0

#: Added to that grace period to cover the teardown and report writing
#: that follow it. A separate term rather than a floor over the two, so
#: raising a grace period raises the requirement instead of vanishing
#: into it.
TERMINATION_SAFETY_MARGIN_SECONDS: typ.Final[float] = 60.0

#: Build time inside a `cargo` invocation before nextest starts its own
#: clock. Only used if a `global-timeout` appears.
COLD_BUILD_ALLOWANCE_SECONDS: typ.Final[float] = 10 * 60.0


def required_ceiling(budgets: typ.Sequence[float], allowance: float) -> float:
    """Return the smallest acceptable ceiling for one coverage job.

    Three terms. Each coverage step may legitimately spend its whole
    watchdog, so the sum is the floor. The measured work outside those
    windows is added because the job timer covers it and the watchdogs
    do not. The margin is added because a ceiling equal to that sum
    cancels the job at the moment the watchdog would have reported the
    overrun, and the report is the only thing that makes it actionable.

    Parameters
    ----------
    budgets : typ.Sequence[float]
        One watchdog budget per coverage step, in seconds.
    allowance : float
        The measured work outside those windows, in seconds.

    Returns
    -------
    float
        The smallest acceptable ceiling, in seconds.
    """
    return sum(budgets) + allowance + CEILING_MARGIN_SECONDS
