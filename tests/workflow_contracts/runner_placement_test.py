"""Contract for where ``ci.yml``'s jobs run.

The Linux ``build-test`` leg runs on Ubicloud's ``ubicloud-standard-4``:
on GitHub-hosted runners its queue wait reached 29 minutes against a
27-minute wall. A fork's pull request cannot obtain an Ubicloud runner,
so that leg falls back to ``ubuntu-latest`` for forks. The Windows leg
and every packaging leg stay GitHub-hosted.

The fallback is read by position. An expression that merely names a
hosted and an Ubicloud label somewhere passes with its arms swapped,
which sends forks to a runner they cannot obtain. The check names are
held independent of the runner, because a name built from an
event-dependent runner exists on only one of a fork's and a branch's
pull requests, and a required context cannot follow it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import typing as typ

import pytest
from workflow_reading import read_workflows, workflow_jobs

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from workflow_reading import WorkflowDocument

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"

#: The fork fallback, by position: condition, fork arm, owned arm.
FORK_CONDITION: typ.Final[str] = "github.event.pull_request.head.repo.fork"
FORK_ARM: typ.Final[str] = "matrix.fork-runner"
OWNED_ARM: typ.Final[str] = "matrix.runner"

#: Each build-test leg, exactly: its runner and its fork's runner.
#: The Linux size is a measured decision, sized for disk rather than wall
#: time. On ``ubicloud-standard-2`` the leg ran out of disk in both runs
#: (36308332712 and 36354295154); the second fell to 791 MB free during the
#: first coverage pass. Naming the label here means a change of size has to
#: change this contract, not just the workflow.
LINUX_RUNNER: typ.Final[str] = "ubicloud-standard-4"

BUILD_TEST_LEGS: typ.Final[dict[str, dict[str, str]]] = {
    "linux": {"runner": LINUX_RUNNER, "fork-runner": "ubuntu-latest"},
    "windows": {"runner": "windows-latest", "fork-runner": "windows-latest"},
}

BUILD_TEST_NAME: typ.Final[str] = "build-test (${{ matrix.platform }})"

#: The Ubicloud cache-credentials step, which only an Ubicloud runner can serve.
CREDENTIALS_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/export-ubicloud-cache-credentials@"
)
SELF_HOSTED: typ.Final[str] = "${{ runner.environment == 'self-hosted' }}"

_FALLBACK = re.compile(r"^\$\{\{\s*(?P<cond>\S+)\s*&&\s*(?P<fork>\S+)\s*\|\|\s*(?P<owned>\S+)\s*\}\}$")


@pytest.fixture(scope="module")
def ci() -> WorkflowDocument:
    """Return ``ci.yml``, parsed once.

    Returns
    -------
    WorkflowDocument
        The parsed workflow.
    """
    return read_workflows(WORKFLOWS)["ci.yml"]


def _build_test(ci: WorkflowDocument) -> dict[str, object]:
    """Return the ``build-test`` job."""
    return workflow_jobs(ci)["build-test"]


def _legs(job: dict[str, object]) -> dict[str, dict[str, object]]:
    """Return a job's matrix ``include`` entries keyed by platform, one each."""
    strategy = job.get("strategy") or {}
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else {}
    include = matrix.get("include", []) if isinstance(matrix, dict) else []
    entries = [entry for entry in include if isinstance(entry, dict)]
    platforms = [str(entry.get("platform")) for entry in entries]
    duplicates = sorted({name for name in platforms if platforms.count(name) > 1})
    # A dict comprehension would keep only the last duplicate and hide a leg.
    assert not duplicates, f"the matrix repeats platform entries: {duplicates}"
    return dict(zip(platforms, entries, strict=True))


def _is_ubicloud(label: object) -> bool:
    """Return whether a runner label names an Ubicloud runner."""
    return str(label).startswith("ubicloud")


def test_the_fork_fallback_is_read_by_position(ci: WorkflowDocument) -> None:
    """A fork runs on the fork arm; everything else on the owned arm."""
    runs_on = " ".join(str(_build_test(ci).get("runs-on", "")).split())
    parsed = _FALLBACK.match(runs_on)
    assert parsed, f"build-test's runs-on is not a fork fallback: {runs_on!r}"
    assert parsed.group("cond") == FORK_CONDITION, (
        f"the fallback must be conditioned on {FORK_CONDITION}; it reads {parsed.group('cond')!r}"
    )
    assert parsed.group("fork") == FORK_ARM, (
        f"forks must take {FORK_ARM}; they take {parsed.group('fork')!r}"
    )
    assert parsed.group("owned") == OWNED_ARM, (
        f"owned pull requests must take {OWNED_ARM}; they take {parsed.group('owned')!r}"
    )


def test_each_build_test_leg_runs_where_it_was_measured(ci: WorkflowDocument) -> None:
    """Linux on standard-4 with a hosted fork arm; Windows hosted on both arms."""
    legs = _legs(_build_test(ci))
    assert set(legs) == set(BUILD_TEST_LEGS), f"build-test's legs are {sorted(legs)}"
    for platform, expected in BUILD_TEST_LEGS.items():
        actual = {key: legs[platform].get(key) for key in expected}
        assert actual == expected, f"the {platform} leg runs on {actual}, not {expected}"


def test_no_fork_is_sent_to_ubicloud(ci: WorkflowDocument) -> None:
    """Every leg's fork arm is a GitHub-hosted label."""
    for platform, leg in _legs(_build_test(ci)).items():
        assert not _is_ubicloud(leg.get("fork-runner")), (
            f"the {platform} leg sends forks to {leg.get('fork-runner')!r}"
        )


def test_the_check_names_do_not_follow_the_runner(ci: WorkflowDocument) -> None:
    """Required contexts are spelt from the platform, never the runner."""
    assert _build_test(ci).get("name") == BUILD_TEST_NAME, (
        f"build-test must be named {BUILD_TEST_NAME!r}; it is {_build_test(ci).get('name')!r}"
    )
    for job_id, job in workflow_jobs(ci).items():
        name = str(job.get("name", ""))
        for runner_reference in ("matrix.runner", "matrix.fork-runner", "matrix.os", "runner."):
            assert runner_reference not in name, (
                f"{job_id}'s name {name!r} interpolates {runner_reference}"
            )


def test_the_moved_leg_has_a_ceiling(ci: WorkflowDocument) -> None:
    """An Ubicloud runner is billed until the job ends, so the job is bounded."""
    # The workflow reader keeps scalars as written, so the ceiling is text.
    ceiling = str(_build_test(ci).get("timeout-minutes", ""))
    assert ceiling.isdigit(), f"build-test's timeout-minutes must be a number; it is {ceiling!r}"
    assert int(ceiling) > 0, f"build-test's timeout-minutes must be positive; it is {ceiling!r}"


def test_only_ubicloud_runners_export_the_cache_credentials(ci: WorkflowDocument) -> None:
    """The credentials step runs on Ubicloud alone, and before Setup Rust.

    On a hosted runner there is no proxy and the action fails closed, so
    an unguarded step fails the Windows leg and every fork's Linux leg.
    It has to precede Setup Rust, which starts the sccache server that
    binds its backend once.
    """
    steps = [step for step in _build_test(ci).get("steps", []) if isinstance(step, dict)]
    positions = [
        index
        for index, step in enumerate(steps)
        if str(step.get("uses", "")).startswith(CREDENTIALS_ACTION)
    ]
    assert len(positions) == 1, f"build-test exports the credentials {len(positions)} times"
    (index,) = positions
    guard = " ".join(str(steps[index].get("if", "")).split())
    assert guard == SELF_HOSTED, f"the credentials step is guarded on {guard!r}"
    setup_rust = next(
        (position for position, step in enumerate(steps)
         if "/setup-rust@" in str(step.get("uses", ""))),
        None,
    )
    assert setup_rust is not None, "build-test no longer runs setup-rust"
    assert index < setup_rust, "the credentials must be exported before Setup Rust"


def test_packaging_stays_hosted(ci: WorkflowDocument) -> None:
    """The packaging legs were not moved: their queue wait was not material."""
    job = workflow_jobs(ci)["binstall-packaging"]
    strategy = job.get("strategy") or {}
    include = strategy.get("matrix", {}).get("include", []) if isinstance(strategy, dict) else []
    runners = [entry.get("os") for entry in include if isinstance(entry, dict)]
    assert runners, "binstall-packaging declares no legs"
    assert not any(_is_ubicloud(runner) for runner in runners), (
        f"a packaging leg moved to Ubicloud: {runners}"
    )


#: The discard that clears the lint trees before coverage on Linux.
DISCARD_COMMAND: typ.Final[str] = "python3 scripts/discard_build_trees.py target debug doc dylint"
LINUX_ONLY: typ.Final[str] = "${{ matrix.platform == 'linux' }}"


def test_the_lint_trees_go_before_coverage(ci: WorkflowDocument) -> None:
    """On Linux the lint build trees are discarded before the first coverage step.

    The first run on ``ubicloud-standard-2`` died of a full disk during
    coverage. The discard runs exactly this command, on Linux, ahead of
    every coverage step.
    """
    steps = [step for step in _build_test(ci).get("steps", []) if isinstance(step, dict)]
    discards = [
        index for index, step in enumerate(steps)
        if " ".join(str(step.get("run", "")).split()) == DISCARD_COMMAND
    ]
    assert len(discards) == 1, f"build-test runs the discard {len(discards)} times"
    (index,) = discards
    guard = " ".join(str(steps[index].get("if", "")).split())
    assert guard == LINUX_ONLY, f"the discard is guarded on {guard!r}"
    coverage = [
        position for position, step in enumerate(steps)
        if "/generate-coverage@" in str(step.get("uses", ""))
    ]
    assert coverage, "build-test runs no coverage step"
    assert index < min(coverage), "the lint trees must go before coverage starts"


#: Every ``build-test`` step whose ``if`` follows the platform, exactly. The
#: ratchet input is held by the shared CV-005 library and
#: ``coverage_lane_facts_test.py``. A step keyed on the
#: wrong platform, or on the old runner label, would skip or run the wrong leg
#: with no failure to show for it.
PLATFORM_CONDITIONS: typ.Final[dict[str, str]] = {
    "Install mdtablefix": LINUX_ONLY,
    "Check formatting": LINUX_ONLY,
    "Check Rust formatting": "${{ matrix.platform != 'linux' }}",
    "Install Whitaker": LINUX_ONLY,
    "Disk sample (before lint)": "${{ always() && matrix.platform == 'linux' }}",
    "Lint": LINUX_ONLY,
    "Lint (Clippy only)": "${{ matrix.platform != 'linux' }}",
    "Workflow contract tests": LINUX_ONLY,
    "Disk sample (before discard)": "${{ always() && matrix.platform == 'linux' }}",
    "Discard lint build trees": LINUX_ONLY,
    "Disk sample (before coverage)": "${{ always() && matrix.platform == 'linux' }}",
    "Disk sample (after coverage with serde_saphyr)": (
        "${{ always() && matrix.platform == 'linux' }}"
    ),
    "Disk sample (after coverage without serde_saphyr)": (
        "${{ always() && matrix.platform == 'linux' }}"
    ),
    "Merge coverage results": LINUX_ONLY,
    "Validate PowerShell wrapper": "${{ matrix.platform == 'windows' }}",
}


def _steps(ci: WorkflowDocument) -> list[dict[str, object]]:
    """Return ``build-test``'s steps, failing clearly if they are not a list of mappings."""
    steps = _build_test(ci).get("steps")
    assert isinstance(steps, list), f"build-test's steps are not a list: {type(steps).__name__}"
    assert all(isinstance(step, dict) for step in steps), "a build-test step is not a mapping"
    return steps


def _condition(step: dict[str, object]) -> str:
    """Return a step's ``if`` with whitespace normalised, or an empty string."""
    return " ".join(str(step.get("if", "")).split())


@pytest.mark.parametrize(("name", "expected"), sorted(PLATFORM_CONDITIONS.items()))
def test_each_migrated_step_keeps_its_exact_platform_condition(
    ci: WorkflowDocument, name: str, expected: str
) -> None:
    """A step keyed on the platform runs on that leg and no other."""
    matching = [step for step in _steps(ci) if step.get("name") == name]
    assert len(matching) == 1, f"build-test has {len(matching)} steps named {name!r}"
    assert _condition(matching[0]) == expected, (
        f"{name!r} is conditioned on {_condition(matching[0])!r}, not {expected!r}"
    )


def test_no_other_step_is_keyed_on_the_platform_or_a_runner_label(ci: WorkflowDocument) -> None:
    """The set of platform-keyed steps is closed, and none reads a runner label.

    This is the narrow half of the contract above: a new step conditioned on
    the platform must be added to the table on purpose, and none may go back
    to testing ``matrix.os`` or ``runner.os`` labels the rename retired.
    """
    keyed = {
        str(step.get("name"))
        for step in _steps(ci)
        if "matrix.platform" in _condition(step)
    }
    assert keyed == set(PLATFORM_CONDITIONS), (
        f"platform-keyed steps differ from the table: {sorted(keyed ^ set(PLATFORM_CONDITIONS))}"
    )
    stale = [
        str(step.get("name"))
        for step in _steps(ci)
        if "matrix.os" in _condition(step) or "ubuntu-latest" in _condition(step)
    ]
    assert not stale, f"steps still key on a runner label: {stale}"


def test_the_discard_follows_lint_and_precedes_both_coverage_passes(ci: WorkflowDocument) -> None:
    """The discard can only free the lint output if it runs after lint.

    It must also precede both coverage steps, or one pass would build beside
    the trees it was meant to replace.
    """
    steps = _steps(ci)
    names = [str(step.get("name")) for step in steps]
    discard = names.index("Discard lint build trees")
    assert names.index("Lint") < discard, "the discard must run after Lint"
    coverage = [
        index for index, step in enumerate(steps) if "/generate-coverage@" in str(step.get("uses", ""))
    ]
    assert len(coverage) == 2, f"build-test runs {len(coverage)} coverage steps, not two"
    assert discard < min(coverage), "the discard must run before both coverage passes"
