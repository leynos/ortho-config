"""Unit and property coverage for the timeout readings.

The ordering contract compares four numbers, and its assertions over the
repository's own workflows are satisfied by several plausibly wrong
readings. Every ceiling here sits well above its requirement, so a
missing term in the derivation changes nothing observable; the two
coverage steps always carry the same watchdog, so a reading that took
one of them would agree with a correct one.

These tests drive the readings with synthetic workflows and synthetic
nextest configurations instead, where a wrong reading has nowhere to
hide, and they fix the error paths so a malformed input is refused
rather than turned into a plausible number.
"""

from __future__ import annotations

import typing as typ

import pytest
from hypothesis import given
from hypothesis import strategies as st
from coverage_lanes import coverage_jobs_of
from timeout_budgets import (
    CEILING_MARGIN_SECONDS,
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
    WATCHDOG_VARIABLE,
    grace_period,
    largest_test_allowance,
    required_ceiling,
    seconds,
    termination_allowance,
)

#: The units nextest accepts, with their length in seconds.
UNITS: typ.Final[dict[str, float]] = {"ms": 0.001, "s": 1.0, "m": 60.0, "h": 3600.0}

COVERAGE_STEP: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage@abc123"
)

whole_numbers = st.integers(min_value=1, max_value=10_000)
units = st.sampled_from(sorted(UNITS))
multipliers = st.integers(min_value=1, max_value=20)


@given(value=whole_numbers, unit=units)
def test_every_unit_scales_its_value(value: int, unit: str) -> None:
    """A duration is its number times the length of its unit.

    The unit table decides every comparison the contract makes, so a
    single wrong entry would leave each of them an inequality between
    two plausible numbers rather than a check.
    """
    assert seconds(f"{value}{unit}") == pytest.approx(value * UNITS[unit]), (
        f"{value}{unit} must scale by the length of its unit"
    )


@pytest.mark.parametrize(
    "duration",
    ["", "300", "s", "300 sec", "five minutes", "-30s", "30d"],
    ids=[
        "empty",
        "no-unit",
        "no-value",
        "an-unsupported-spelling",
        "words",
        "negative",
        "days-are-not-a-nextest-unit",
    ],
)
def test_an_unreadable_duration_is_refused(duration: str) -> None:
    """A duration nextest would reject must not become a number.

    Returning something plausible would put a comparison against a
    budget nextest never applies, and the contract would pass while the
    ordering it claims to hold did not.
    """
    with pytest.raises(AssertionError):
        seconds(duration)


@given(
    budgets=st.lists(
        st.tuples(whole_numbers, units, multipliers), min_size=1, max_size=8
    )
)
def test_the_largest_budget_is_the_largest_product(
    budgets: list[tuple[int, str, int]],
) -> None:
    """Every `slow-timeout` counts, and each counts as a product.

    A reading that took the first entry, or the largest period without
    its multiplier, agrees with a correct one whenever the two happen to
    coincide. Over generated configurations they stop coinciding.
    """
    config = "\n".join(
        f'slow-timeout = {{ period = "{value}{unit}", terminate-after = {times} }}'
        for value, unit, times in budgets
    )
    expected = max(value * UNITS[unit] * times for value, unit, times in budgets)
    assert largest_test_allowance(config) == pytest.approx(expected), (
        "the largest budget is the largest period times its own multiplier"
    )


@given(periods=st.lists(st.tuples(whole_numbers, units), min_size=1, max_size=6))
def test_the_termination_allowance_tracks_the_largest_grace_period(
    periods: list[tuple[int, str]],
) -> None:
    """The allowance is the largest grace period plus the fixed margin."""
    config = "\n".join(
        f'slow-timeout = {{ period = "1s", grace-period = "{value}{unit}" }}'
        for value, unit in periods
    )
    largest = max(value * UNITS[unit] for value, unit in periods)
    assert grace_period(config) == pytest.approx(largest), (
        "the largest configured grace period governs"
    )
    assert termination_allowance(config) == pytest.approx(
        largest + TERMINATION_SAFETY_MARGIN_SECONDS
    ), "the allowance is the grace period plus the margin, not the larger"


@given(text=st.text(max_size=40).filter(lambda body: "grace-period" not in body))
def test_an_unconfigured_grace_period_falls_back_to_nextest_s_default(
    text: str,
) -> None:
    """Assuming zero would understate what nextest needs to stop a run."""
    assert grace_period(text) == pytest.approx(NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS), (
        "an absent grace period must fall back to nextest's default"
    )


@given(
    budgets=st.lists(st.floats(min_value=1.0, max_value=7200.0), max_size=4),
    allowance=st.floats(min_value=0.0, max_value=7200.0),
)
def test_the_required_ceiling_is_monotone_in_every_term(
    budgets: list[float], allowance: float
) -> None:
    """More watchdogs, or more work outside them, can only ask for more.

    A derivation that took the largest watchdog rather than their sum,
    or dropped the allowance, would still be an increasing function of
    something, so this pairs the shape with the exact value below.
    """
    required = required_ceiling(budgets, allowance)
    assert required == pytest.approx(
        sum(budgets) + allowance + CEILING_MARGIN_SECONDS
    ), "the requirement is the watchdogs, the allowance, and the margin"
    assert required >= sum(budgets) + allowance, (
        "the requirement can never fall below the work it has to contain"
    )
    assert required_ceiling([*budgets, 60.0], allowance) > required, (
        "adding a watchdog must raise the requirement"
    )


def _workflow(
    *, steps: int = 2, ceiling: int | None = 135, watchdog: int | None = 1800
) -> dict[str, dict[str, object]]:
    """Return one synthetic workflow containing one coverage job."""
    job: dict[str, object] = {
        "steps": [
            {"name": f"cover {index}", "uses": COVERAGE_STEP} for index in range(steps)
        ]
    }
    if ceiling is not None:
        job["timeout-minutes"] = ceiling
    if watchdog is not None:
        job["env"] = {WATCHDOG_VARIABLE: watchdog}
    return {"jobs": {"build-test": job}}


def test_a_job_is_read_from_a_synthetic_workflow() -> None:
    """The reading is driven without touching the repository."""
    (job,) = coverage_jobs_of({"ci.yml": _workflow()})
    assert job.workflow == "ci.yml", "the entry carries its file name"
    assert job.job == "build-test", "the entry carries its job identifier"
    assert job.steps == 2, "both coverage steps are counted"
    assert job.watchdogs == (1800.0, 1800.0), "the job's watchdog reaches both steps"
    assert job.job_timeout == pytest.approx(8100.0), "minutes convert to seconds"


def test_a_job_without_a_ceiling_reads_as_none_rather_than_absent() -> None:
    """An absent entry would make the ceiling assertion skip the lane.

    That is the failure this contract exists to prevent, so a missing
    ceiling has to survive the reading as `None` rather than dropping
    the job from the list.
    """
    (job,) = coverage_jobs_of({"ci.yml": _workflow(ceiling=None)})
    assert job.job_timeout is None, "a job with no timeout-minutes reads as None"


def test_a_step_without_a_watchdog_reads_as_none_rather_than_absent() -> None:
    """The same argument, one tier in.

    A lane that lost its watchdog override silently takes the action's
    1,800 s default, so the reading has to show `None` rather than
    omitting the step and shrinking the requirement.
    """
    (job,) = coverage_jobs_of({"ci.yml": _workflow(watchdog=None)})
    assert job.watchdogs == (None, None), "an unset watchdog reads as None"


@pytest.mark.parametrize(
    "document",
    [
        pytest.param({"jobs": {"build": "not a mapping"}}, id="a-job-that-is-a-scalar"),
        pytest.param({"jobs": {}}, id="no-jobs"),
        pytest.param({}, id="an-empty-document"),
        pytest.param({"jobs": {"build": {"steps": "not a list"}}}, id="steps-as-text"),
        pytest.param(
            {"jobs": {"build": {"steps": [{"run": "make test"}]}}},
            id="no-coverage-step",
        ),
    ],
)
def test_a_malformed_or_unrelated_workflow_yields_no_job(
    document: dict[str, object],
) -> None:
    """A shape the reading does not expect must not become a job.

    Raising here would fail the whole contract on an unrelated workflow;
    inventing a job would assert budgets nobody wrote.
    """
    assert not coverage_jobs_of({"ci.yml": document}), (
        f"{document!r} declares no coverage job"
    )


@pytest.mark.parametrize(
    "document",
    [
        pytest.param({"jobs": "not a mapping"}, id="jobs-is-a-scalar"),
        pytest.param({"jobs": ["build"]}, id="jobs-is-a-list"),
        pytest.param({"jobs": {"build": "not a mapping"}}, id="a-job-is-a-scalar"),
    ],
    ids=str,
)
def test_a_malformed_jobs_container_yields_no_lane(
    document: dict[str, object],
) -> None:
    """A shape the reading does not expect is not a coverage lane.

    A non-empty scalar reaches `.items()` and raises, which fails the
    whole contract on a workflow that has nothing to do with coverage.
    Reporting no lane is the honest answer: the document declares none
    that this contract can see, and a workflow that will not parse is
    the loader's business rather than the budgets'.
    """
    assert not coverage_jobs_of({"ci.yml": document}), (
        f"{document!r} declares no coverage job"
    )


@pytest.mark.parametrize(
    ("ceiling", "expected"),
    [
        pytest.param(135, 8100.0, id="a-whole-number-of-minutes"),
        pytest.param("135", 8100.0, id="minutes-as-a-string"),
        pytest.param("soon", None, id="not-a-number"),
        pytest.param(True, None, id="a-boolean"),
        pytest.param([135], None, id="a-list"),
        pytest.param(None, None, id="absent"),
    ],
)
def test_an_unreadable_ceiling_reads_as_absent(
    ceiling: object, expected: float | None
) -> None:
    """A ceiling that is not a number of minutes is not a ceiling.

    Converting it directly raised during collection, so a workflow with
    a mistyped `timeout-minutes` failed the contract with a Python
    fault rather than with the assertion that the job declares no
    usable ceiling. Reading it as absent puts the failure where a
    maintainer can act on it.
    """
    (job,) = coverage_jobs_of({"ci.yml": _workflow(ceiling=ceiling)})

    if expected is None:
        assert job.job_timeout is None, f"{ceiling!r} is not a usable ceiling"
    else:
        assert job.job_timeout == pytest.approx(expected), f"{ceiling!r} is {expected}s"


@pytest.mark.parametrize(
    "environment",
    ["not a mapping", ["RUN_RUST_CARGO_WAIT_TIMEOUT=1800"], 1800],
    ids=["a-string", "a-list", "a-number"],
)
def test_a_malformed_environment_reads_as_setting_nothing(
    environment: object,
) -> None:
    """An `env` that is not a mapping sets no watchdog.

    Raising here would fail the contract on the shape of an unrelated
    field; inventing a budget would certify a lane nobody bounded.
    """
    document = {
        "jobs": {
            "build-test": {
                "timeout-minutes": 135,
                "env": environment,
                "steps": [{"uses": COVERAGE_STEP}],
            }
        }
    }

    (job,) = coverage_jobs_of({"ci.yml": typ.cast("dict[str, typ.Any]", document)})

    assert job.watchdogs == (None,), (
        f"an env of {environment!r} sets no watchdog, so the lane inherits "
        f"the action's default and must read as unset"
    )
