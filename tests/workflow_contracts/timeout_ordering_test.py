"""Contract for the timers that can end a test run.

Four independent budgets can end a coverage lane, each set somewhere
different, and they only work if each sits above the one inside it.
Three of the four are set here: a per-test ``slow-timeout`` in
``.config/nextest.toml``, the shared coverage action's wall-clock
watchdog on each ``cargo`` invocation, and the job's own
``timeout-minutes``.

The outermost tier was missing entirely until this contract was written.
Neither coverage job declared ``timeout-minutes``, so both inherited
GitHub's six-hour default, and the Windows leg already runs for 86
minutes. A hang there cost six hours of a paid runner before anything
stopped it.

This repository has a wrinkle the canonical rule does not spell out:
each coverage job runs the action **twice**, once with `serde_saphyr`
and once without. Each invocation gets its own watchdog, so the job must
be able to contain both budgets before it contains anything else. The
requirement is therefore the watchdog multiplied by the number of
coverage steps in that job, plus the measured work outside them.

See "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md``, and the canonical wording in
`leynos/shared-actions`' `generate-coverage` README.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

import pytest
from coverage_lanes import CoverageJob, coverage_jobs_of
from timeout_budgets import (
    CEILING_MARGIN_SECONDS,
    COLD_BUILD_ALLOWANCE_SECONDS,
    COVERAGE_ACTION,
    DEFAULT_OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS,
    NEXTEST_CONFIG,
    OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS,
    WATCHDOG_VARIABLE,
    global_timeout,
    largest_test_allowance,
    required_ceiling,
    termination_allowance,
)

#: The condition each coverage lane legitimately carries, keyed by
#: workflow and job, as the step's ``if`` and its job's.
#:
#: A skipped step runs no `cargo`, so its watchdog never arms and every
#: assertion below says nothing about it. `if: false` on either would
#: leave a lane that looks bounded and is not. The values are pinned
#: rather than merely tolerated, because a lane gaining, losing or
#: changing a condition changes when it runs at all.
REQUIRED_CONDITIONS: typ.Final[dict[tuple[str, str], tuple[object, object]]] = {
    ("ci.yml", "build-test"): (None, None),
    ("coverage-main.yml", "coverage-upload"): (None, None),
}

#: How many coverage steps each job is required to run, pinned by
#: coordinate. The ceiling requirement is derived from the number of
#: steps found, so deleting one lowers the requirement and every
#: assertion still passes while the lane loses half its instrumentation.
REQUIRED_COVERAGE_STEPS: typ.Final[dict[tuple[str, str], int]] = {
    ("ci.yml", "build-test"): 2,
    ("coverage-main.yml", "coverage-upload"): 2,
}

#: How far a ceiling must sit above its requirement rather than merely
#: reaching it. A ceiling equal to the sum it has to contain cancels the
#: job at the moment the innermost timer would have reported the
#: overrun, so the report is lost exactly when it is wanted. Fifteen
#: minutes is the margin the other lanes in this estate carry.
@pytest.fixture(scope="module")
def nextest_config() -> str:
    """Return the nextest configuration file's text.

    Returns
    -------
    str
        The file's contents.
    """
    return NEXTEST_CONFIG.read_text(encoding="utf-8")


@pytest.fixture(scope="module")
def coverage_jobs() -> tuple[CoverageJob, ...]:
    """Return every coverage-invoking job, read once for the module.

    Returns
    -------
    tuple[CoverageJob, ...]
        One entry per coverage-invoking job.
    """
    return coverage_jobs_of()


def test_the_coverage_action_is_invoked_somewhere(
    coverage_jobs: tuple[CoverageJob, ...],
) -> None:
    """The contract needs a job to assert against.

    A repin or a rename that stopped the coordinate matching would
    otherwise turn every assertion below into a vacuous pass over an
    empty list, and the loss would look exactly like success.
    """
    assert coverage_jobs, (
        f"no workflow job uses {COVERAGE_ACTION}; either coverage moved or "
        f"this contract stopped recognizing it"
    )


def test_each_coverage_job_runs_the_steps_it_is_meant_to() -> None:
    """The ceiling's requirement is derived from the steps that are there.

    ``required`` is the sum of the watchdogs found plus the allowance, so
    deleting one of a job's two coverage steps lowers the requirement by
    1,800 s and every timing assertion still passes, while the lane
    silently measures half of what it did. Pinning the count by
    coordinate is what makes that deletion fail.
    """
    found = {(job.workflow, job.job): job.steps for job in coverage_jobs_of()}
    wrong = {
        coordinate: (expected, found.get(coordinate))
        for coordinate, expected in REQUIRED_COVERAGE_STEPS.items()
        if found.get(coordinate) != expected
    }
    assert not wrong, (
        f"these jobs do not run the number of coverage steps recorded in the "
        f"developers' guide, as expected versus found: {wrong}"
    )


def test_every_coverage_step_runs_under_an_explicit_watchdog(
    coverage_jobs: tuple[CoverageJob, ...],
) -> None:
    """The default is invisible, so every step must write it down.

    The action kills `cargo` after 1,800 s unless told otherwise, and the
    value here equals that default, which makes writing it down more
    important rather than less: an accidental deletion would change
    nothing observable until the run it killed.
    """
    missing = [
        f"{job}: step {index + 1} of {job.steps}"
        for job in coverage_jobs
        for index, watchdog in enumerate(job.watchdogs)
        if watchdog is None
    ]
    assert not missing, (
        f"these coverage steps do not set {WATCHDOG_VARIABLE} and so inherit "
        f"the shared action's undocumented default: {missing}"
    )


def test_the_job_ceiling_contains_every_watchdog_and_the_work_around_them(
    coverage_jobs: tuple[CoverageJob, ...],
) -> None:
    """Tier four must not pre-empt tier three, for any of the invocations.

    Each coverage step gets its own watchdog, so a job running the action
    twice can legitimately spend both budgets, and its ceiling has to
    contain the sum rather than one of them. The clocks do not start
    together either: the job timer starts before the checkout and runs
    through the cache saves afterwards, which on Windows are the largest
    thing in the job outside the coverage steps themselves.

    A ceiling merely above one watchdog cancels the job partway through
    the second invocation, and a cancellation discards the log that would
    have explained it.
    """
    for job in coverage_jobs:
        budgets = [watchdog for watchdog in job.watchdogs if watchdog is not None]
        assert len(budgets) == job.steps, str(job)
        allowance = OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS.get(
            job.workflow, DEFAULT_OUTSIDE_WATCHDOG_ALLOWANCE_SECONDS
        )
        required = required_ceiling(budgets, allowance)
        assert job.job_timeout is not None, (
            f"{job} runs {job.steps} watchdog-bounded cargo invocation(s) in a "
            f"job with no timeout-minutes; the outermost tier is missing and "
            f"GitHub's six-hour default applies"
        )
        assert job.job_timeout >= required, (
            f"{job} has a ceiling of {job.job_timeout:.0f}s, below the "
            f"{required:.0f}s needed to contain {job.steps} watchdog(s) "
            f"totalling {sum(budgets):.0f}s, {allowance:.0f}s of measured "
            f"work outside them, and a {CEILING_MARGIN_SECONDS:.0f}s margin "
            f"above that sum; an overrun would be cancelled rather than "
            f"reported"
        )


def test_a_whole_run_budget_would_sit_inside_each_watchdog(
    coverage_jobs: tuple[CoverageJob, ...], nextest_config: str
) -> None:
    """Tier three must not pre-empt tier two, if tier two appears.

    No ``global-timeout`` is set today, so this asserts nothing about the
    current tree and is not a licence to leave it that way: the guide
    records the gap. What it does is bind the value the moment one is
    added, so it arrives above the largest per-test allowance and inside
    the watchdog rather than merely somewhere.
    """
    whole_run = global_timeout(nextest_config)
    if whole_run is None:
        pytest.skip("no global-timeout is set; the guide records this as a gap")
    largest = largest_test_allowance(nextest_config)
    assert whole_run > largest, (
        f"the {whole_run:.0f}s global-timeout is not above the {largest:.0f}s "
        f"largest per-test allowance; the run would end before that test "
        f"could use its budget"
    )
    required = (
        whole_run
        + termination_allowance(nextest_config)
        + COLD_BUILD_ALLOWANCE_SECONDS
    )
    for job in coverage_jobs:
        for index, watchdog in enumerate(job.watchdogs):
            assert watchdog is not None, str(job)
            assert watchdog >= required, (
                f"{job} step {index + 1} sets a {watchdog:.0f}s watchdog, "
                f"below the {required:.0f}s needed to cover the "
                f"{whole_run:.0f}s whole-run budget, nextest's termination "
                f"procedure, and a cold build"
            )


def test_each_coverage_lane_carries_the_condition_it_is_meant_to(
    coverage_jobs: tuple[CoverageJob, ...],
) -> None:
    """A skipped step runs no `cargo`, so its watchdog never arms.

    Every assertion above reads a lane's declared budgets and says
    nothing about whether the step runs. `if: false` on the step or on
    its job would leave a lane that looks bounded and is not, and this
    contract would certify it. So would a plausible condition that
    quietly excluded the event the lane exists for.

    Neither coverage lane here carries one today, so the pin is that
    they carry none: adding a condition has to change this contract and
    the guide with it. The coordinates are compared both ways first, so
    a new lane with no entry here fails rather than passing unexamined,
    and a lane that disappeared fails rather than being skipped.

    Proved by mutation: `if: false` on the coverage step, the same on
    its job, a push-only condition, and a coordinate dropped from
    ``REQUIRED_CONDITIONS`` each fail this test.
    """
    found = {(job.workflow, job.job): job.conditions for job in coverage_jobs}
    assert set(found) == set(REQUIRED_CONDITIONS), (
        f"the coverage lanes are not the ones this contract pins: "
        f"unlisted {sorted(set(found) - set(REQUIRED_CONDITIONS))}, missing "
        f"{sorted(set(REQUIRED_CONDITIONS) - set(found))}; a lane with no "
        f"entry here is a lane whose condition nobody has judged"
    )
    wrong = {
        coordinate: (expected, found[coordinate])
        for coordinate, expected in REQUIRED_CONDITIONS.items()
        if set(found[coordinate]) != {expected}
    }
    assert not wrong, (
        f"these coverage lanes do not carry the conditions the developers' "
        f"guide records, as expected versus found: {wrong}; a lane that is "
        f"skipped runs no cargo, so its watchdog never arms"
    )
