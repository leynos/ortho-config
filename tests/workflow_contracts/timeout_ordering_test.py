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

import re
import typing as typ
from pathlib import Path

import pytest
import yaml

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

_DURATION: typ.Final[re.Pattern[str]] = re.compile(
    r"^\s*(?P<value>\d+(?:\.\d+)?)\s*(?P<unit>ms|s|m|h)\s*$"
)

_UNIT_SECONDS: typ.Final[dict[str, float]] = {
    "ms": 0.001,
    "s": 1.0,
    "m": 60.0,
    "h": 3600.0,
}

#: One `slow-timeout` inline table, captured whole so the period and the
#: multiplier that scales it are read together. nextest warns once per
#: `period` and terminates after `terminate-after` of them, so the budget
#: is their product; reading the period alone understates it fivefold
#: here.
_SLOW_TIMEOUT: typ.Final[re.Pattern[str]] = re.compile(
    r"slow-timeout\s*=\s*\{(?P<body>[^}]*)\}"
)

_GRACE_PERIOD: typ.Final[re.Pattern[str]] = re.compile(
    r'grace-period\s*=\s*"([^"]+)"'
)


def seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"120s"``.

    Returns
    -------
    float
        The duration in seconds.
    """
    match = _DURATION.match(duration)
    assert match is not None, f"unrecognized nextest duration {duration!r}"
    return float(match["value"]) * _UNIT_SECONDS[match["unit"]]


def largest_test_allowance(config_text: str) -> float:
    """Return the longest a single test may run, in seconds.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The longest per-test budget, period multiplied by
        ``terminate-after``.
    """
    budgets: list[float] = []
    for match in _SLOW_TIMEOUT.finditer(config_text):
        body = match["body"]
        period = re.search(r'period\s*=\s*"([^"]+)"', body)
        assert period is not None, f"slow-timeout without a period: {body!r}"
        terminate = re.search(r"terminate-after\s*=\s*(\d+)", body)
        multiplier = 1 if terminate is None else int(terminate[1])
        budgets.append(seconds(period[1]) * multiplier)
    assert budgets, "nextest.toml must set at least one slow-timeout"
    return max(budgets)


def grace_period(config_text: str) -> float:
    """Return the longest grace period the configuration names, in seconds.

    Read from the configuration rather than fixed, so a profile that
    raised its grace period raises the requirement too. nextest's own
    default applies when none is named, as none is here.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The largest configured grace period, or nextest's default.
    """
    periods = _GRACE_PERIOD.findall(config_text)
    return max(
        (seconds(period) for period in periods),
        default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    )


def termination_allowance(config_text: str) -> float:
    """Return the time nextest may take to stop the run, in seconds.

    Two terms, not one: what nextest promises a test after ``SIGTERM``,
    plus a margin for the teardown and report writing that follow it.
    A single floor over the two would absorb every grace period below
    the margin, so raising one would look free until the run it
    cancelled.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The grace period plus the safety margin.
    """
    return grace_period(config_text) + TERMINATION_SAFETY_MARGIN_SECONDS


def global_timeout(config_text: str) -> float | None:
    """Return the whole-run budget, or None when none is set.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float or None
        The whole-run budget in seconds, or None.
    """
    match = re.search(r'^global-timeout\s*=\s*"([^"]+)"', config_text, re.MULTILINE)
    return None if match is None else seconds(match[1])


class CoverageJob(typ.NamedTuple):
    """One job that invokes the coverage action, with its budgets.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    job : str
        The job's identifier.
    steps : int
        How many coverage steps the job runs. Each gets its own watchdog,
        so the job must contain all of their budgets.
    watchdogs : tuple[float | None, ...]
        The watchdog budget in force for each of those steps, in order,
        with None where neither the step nor the job sets one.
    job_timeout : float or None
        The job's ``timeout-minutes`` in seconds, or None when it
        declares none and so inherits GitHub's six-hour default.
    """

    workflow: str
    job: str
    steps: int
    watchdogs: tuple[float | None, ...]
    job_timeout: float | None
    conditions: tuple[tuple[object, object], ...] = ()

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job`` for this job.
        """
        return f"{self.workflow}:{self.job}"


def _watchdog_of(job: dict[str, typ.Any], step: dict[str, typ.Any]) -> float | None:
    """Return the watchdog budget in force for one step.

    A step's own environment wins over the job's, as GitHub resolves it,
    so a step that overrode the job value is read as it will run rather
    than as the job declares.

    Parameters
    ----------
    job : dict[str, typ.Any]
        The enclosing job.
    step : dict[str, typ.Any]
        The coverage step.

    Returns
    -------
    float or None
        The budget in seconds, or None when neither sets one.
    """
    for owner in (step, job):
        environment = owner.get("env")
        if not isinstance(environment, dict):
            continue
        budget = _budget_from(environment.get(WATCHDOG_VARIABLE))
        if budget is not None:
            return budget
    return None


def _budget_from(raw: object) -> float | None:
    """Return one source's watchdog budget, or None when it sets none."""
    if raw is None:
        return None
    text = str(raw).strip()
    if not text:
        return None
    try:
        seconds = float(text)
    except ValueError:
        return None
    return seconds if seconds > 0 else None


def workflow_documents() -> dict[str, dict[str, typ.Any]]:
    """Return every workflow document in the repository, keyed by name.

    This is the one place the contract touches the filesystem or the
    YAML parser, so an unreadable or unparsable workflow fails here
    rather than inside a budget derivation several frames away.
    Both extensions are read. A coverage lane in the other one would
    otherwise escape every assertion below without failing anything.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        File name to parsed document.
    """
    documents: dict[str, dict[str, typ.Any]] = {}
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
            if isinstance(parsed, dict):
                documents[path.name] = parsed
    return documents


@pytest.fixture(scope="module")
def nextest_config() -> str:
    """Return the nextest configuration file's text.

    Returns
    -------
    str
        The file's contents.
    """
    return NEXTEST_CONFIG.read_text(encoding="utf-8")


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


def _jobs_in(document: dict[str, typ.Any]) -> dict[str, dict[str, typ.Any]]:
    """Return a document's jobs, ignoring anything that is not one."""
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        return {}
    return {
        str(name): job for name, job in jobs.items() if isinstance(job, dict)
    }


#: What GitHub accepts as a number of minutes. `bool` is excluded
#: rather than merely unlisted, because it is an `int` in Python and
#: `timeout-minutes: true` would otherwise read as one minute.
_MINUTE_TYPES: typ.Final[tuple[type, ...]] = (int, float, str)


def _is_minutes(raw: object) -> bool:
    """Return whether a value could be a number of minutes."""
    return isinstance(raw, _MINUTE_TYPES) and not isinstance(raw, bool)


def _ceiling_seconds(raw: object) -> float | None:
    """Return a job's ``timeout-minutes`` in seconds, or None."""
    if not _is_minutes(raw):
        return None
    try:
        return float(typ.cast("int | float | str", raw)) * 60.0
    except ValueError:
        return None


def _coverage_steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return one job's coverage steps, in the order it runs them."""
    steps = job.get("steps")
    if not isinstance(steps, list):
        return []
    return [
        step
        for step in steps
        if isinstance(step, dict) and COVERAGE_ACTION in str(step.get("uses", ""))
    ]


def _coverage_job(
    workflow: str, job_name: str, job: dict[str, typ.Any]
) -> CoverageJob | None:
    """Return one job's budgets, or None when it runs no coverage step."""
    steps = _coverage_steps(job)
    if not steps:
        return None
    raw_timeout = job.get("timeout-minutes")
    return CoverageJob(
        workflow=workflow,
        job=job_name,
        steps=len(steps),
        watchdogs=tuple(_watchdog_of(job, step) for step in steps),
        job_timeout=_ceiling_seconds(raw_timeout),
        conditions=tuple((step.get("if"), job.get("if")) for step in steps),
    )


def coverage_jobs_of(
    documents: dict[str, dict[str, typ.Any]] | None = None,
) -> tuple[CoverageJob, ...]:
    """Return every job invoking the coverage action, with its budgets.

    Jobs are the unit rather than steps, because the ceiling is a job's
    and it has to contain every watchdog inside it. Counting steps is
    what makes the two invocations here visible to the arithmetic.

    The documents are a parameter so the reading can be driven with
    synthetic workflows. Reading the repository's own is the default
    rather than the only option, which keeps the filesystem access and
    the YAML parsing at one named boundary instead of inside the
    derivations.

    Parameters
    ----------
    documents : dict[str, dict[str, typ.Any]] or None
        Parsed workflow documents keyed by file name. When None, the
        repository's own `.github/workflows` is read.

    Returns
    -------
    tuple[CoverageJob, ...]
        One entry per coverage-invoking job.
    """
    if documents is None:
        documents = workflow_documents()
    return tuple(
        found
        for name, document in documents.items()
        for job_name, job in _jobs_in(document).items()
        if (found := _coverage_job(name, str(job_name), job)) is not None
    )


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


def test_the_largest_per_test_allowance_counts_the_multiplier(
    nextest_config: str,
) -> None:
    """``terminate-after`` scales the period; the budget is their product.

    This is the reading that decides every comparison above, and it is
    the one easy to get wrong: a contract reading the period alone would
    report a 120 s largest allowance where the real figure is 600 s.
    """
    largest = largest_test_allowance(nextest_config)
    periods = [
        seconds(match[1])
        for match in re.finditer(r'period\s*=\s*"([^"]+)"', nextest_config)
    ]
    assert largest > max(periods), (
        f"the largest per-test allowance came out as {largest:.0f}s, no more "
        f"than the longest bare period; terminate-after was not counted"
    )


def test_the_termination_allowance_is_the_grace_period_plus_the_margin() -> None:
    """The two terms are added, not maximized over.

    A single floor over the grace period and the margin would absorb
    every grace period below the margin, so adding a thirty-second one
    to this configuration would demand nothing more of the watchdog. No
    ``global-timeout`` is set here, so the ordering assertion that uses
    this reading is skipped entirely, which leaves this test the only
    thing standing behind it.
    """
    assert termination_allowance("") == pytest.approx(
        NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "an unnamed grace period must fall back to nextest's own default"
    configured = termination_allowance(
        'slow-timeout = { period = "60s", grace-period = "30s" }'
    )
    assert configured == pytest.approx(30.0 + TERMINATION_SAFETY_MARGIN_SECONDS), (
        "a grace period below the margin must still raise the allowance; "
        "a maximum over the two terms would have discarded it"
    )
    largest = termination_allowance(
        'slow-timeout = { grace-period = "5s" }\n'
        'slow-timeout = { grace-period = "45s" }'
    )
    assert largest == pytest.approx(45.0 + TERMINATION_SAFETY_MARGIN_SECONDS), (
        "the largest configured grace period governs the allowance"
    )


def test_the_required_ceiling_carries_all_three_terms() -> None:
    """Watchdogs, measured work, and the margin above their sum.

    Every ceiling in this tree already sits well above its requirement,
    so dropping a term from the derivation changes nothing observable
    here and the assertion over the workflows still passes. Driving the
    derivation with controlled numbers is what makes the loss visible.
    """
    assert required_ceiling([1800.0, 1800.0], 3600.0) == pytest.approx(
        3600.0 + 3600.0 + CEILING_MARGIN_SECONDS
    ), "two watchdogs, the allowance, and the margin are all added"
    assert required_ceiling([1800.0], 0.0) == pytest.approx(
        1800.0 + CEILING_MARGIN_SECONDS
    ), "the margin applies even when nothing runs outside the watchdog"
    assert required_ceiling([], 0.0) == pytest.approx(CEILING_MARGIN_SECONDS), (
        "the margin is a term of its own, not a fraction of the others"
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
    the guide with it.
    """
    found = {(job.workflow, job.job): job.conditions for job in coverage_jobs}
    wrong = {
        coordinate: (expected, found.get(coordinate))
        for coordinate, expected in REQUIRED_CONDITIONS.items()
        if not found.get(coordinate) or set(found[coordinate]) != {expected}
    }
    assert not wrong, (
        f"these coverage lanes do not carry the conditions the developers' "
        f"guide records, as expected versus found: {wrong}; a lane that is "
        f"skipped runs no cargo, so its watchdog never arms"
    )
