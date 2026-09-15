"""Contract for the scope of ``lading publish``'s pre-flight.

``lading publish`` runs ``cargo check --workspace --all-targets`` and then
``cargo test`` before it packages anything. In CI that is a second execution
of the workspace this job has already run, so the publish step turns the
pre-flight off. The saving is only defensible while the steps that ran the
workspace are still there and still ahead of it.

This module asserts both halves: the setting, and the premise that justifies
it. Either alone would be a claim about nothing. It also pins what the step
must keep doing, from both ends, because a skip that pruned the packaging as
well would leave the release gate proving nothing.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import itertools
import re
import shlex
import tomllib
import typing as typ
from pathlib import Path

import pytest
import yaml

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CI_WORKFLOW_PATH = REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml"
LADING_CONFIGURATION_PATH = REPOSITORY_ROOT / "lading.toml"
MAKEFILE_PATH = REPOSITORY_ROOT / "Makefile"

#: The job and step this contract is about.
BUILD_TEST_JOB: typ.Final[str] = "build-test"
DRY_RUN_STEP: typ.Final[str] = "Publish dry run"

#: The environment variable that turns lading's pre-flight off, and the one
#: value this workflow may use. lading reads it through Cyclopts, which
#: accepts `1`, `true`, `t`, `yes` and `y` case insensitively, and the
#: matching negatives. The quiet failure is a value from the negative set:
#: the step would succeed, the pre-flight would run in full, and the only
#: evidence would be a step that took minutes longer than the guide says it
#: should. Equality against one agreed spelling rules that out. A value
#: outside the accepted set needs no contract, because lading refuses it and
#: the step fails with the reason in the log.
SKIP_VARIABLE: typ.Final[str] = "LADING_SKIP_PREFLIGHT"
SKIP_ENABLED: typ.Final[str] = "true"

#: The lading.toml key that would skip the pre-flight everywhere, including
#: on a workstation. The workflow sets the variable instead.
SKIP_SETTING: typ.Final[str] = "skip"

#: The steps that execute the workspace. Both are unconditional, so between
#: them they cover every lane the matrix declares, and each must run ahead of
#: the publish step. The skip rests on exactly this.
TEST_STEPS: typ.Final[tuple[str, ...]] = (
    "Test and Measure Coverage (with serde_saphyr)",
    "Test and Measure Coverage (without serde_saphyr)",
)

#: The command the publish step must run, as tokens. The step is the only
#: place packaging is invoked, and the skip removes the pre-flight around it,
#: so what remains has to be the packaging itself.
PACKAGING_COMMAND: typ.Final[tuple[str, ...]] = ("make", "publish-check")

#: The Make target that command names, and the subcommand its recipe must
#: hand to lading. `lading publish` packages and dry-run publishes each crate
#: from its own packaged sources; any other subcommand builds the workspace as
#: a whole and cannot see a symbol missing from a crate root.
PACKAGING_TARGET: typ.Final[str] = "publish-check"
PACKAGING_SUBCOMMAND: typ.Final[str] = "publish"


@pytest.fixture(name="build_test_job", scope="module")
def build_test_job_fixture() -> dict[str, typ.Any]:
    """Return the CI job that both tests and packages."""
    workflow = yaml.safe_load(CI_WORKFLOW_PATH.read_text(encoding="utf-8"))
    jobs = workflow.get("jobs") or {}
    job = jobs.get(BUILD_TEST_JOB)
    assert isinstance(job, dict), (
        f"ci.yml declares no {BUILD_TEST_JOB!r} job; this contract is about the "
        f"job that runs the workspace and then packages it"
    )
    return job


@pytest.fixture(name="lading_configuration", scope="module")
def lading_configuration_fixture() -> dict[str, typ.Any]:
    """Return the parsed lading.toml."""
    return tomllib.loads(LADING_CONFIGURATION_PATH.read_text(encoding="utf-8"))


def steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return a job's steps."""
    return list(job.get("steps") or [])


def step_named(job: dict[str, typ.Any], name: str) -> dict[str, typ.Any]:
    """Return the one step called *name*, failing when it is absent.

    Returns
    -------
    dict
        The named step.
    """
    matches = [step for step in steps(job) if step.get("name") == name]
    assert len(matches) == 1, (
        f"expected exactly one step named {name!r} in {BUILD_TEST_JOB!r}, "
        f"found {len(matches)}"
    )
    return matches[0]


def step_index(job: dict[str, typ.Any], name: str) -> int:
    """Return the position of the step called *name*.

    Returns
    -------
    int
        The step's index within the job.
    """
    for index, step in enumerate(steps(job)):
        if step.get("name") == name:
            return index
    message = f"{BUILD_TEST_JOB!r} declares no step named {name!r}"
    raise AssertionError(message)


def recipe_lines(makefile: str, target: str) -> list[str]:
    r"""Return the recipe lines of one Make target.

    Parameters
    ----------
    makefile : str
        The text of a Makefile.
    target : str
        The target to read.

    Returns
    -------
    list of str
        The target's tab-indented recipe lines, empty when no rule defines
        that target.

    Examples
    --------
    >>> recipe_lines("all:\n\techo hi\n", "all")
    ['echo hi']
    >>> recipe_lines("all:\n\techo hi\n", "absent")
    []
    """
    rule = re.compile(rf"^{re.escape(target)}\s*:(?!=)", re.MULTILINE)
    match = rule.search(makefile)
    if match is None:
        return []
    body = makefile[match.end() :].splitlines()[1:]
    return [line[1:] for line in itertools.takewhile(is_recipe_line, body)]


def is_recipe_line(line: str) -> bool:
    r"""Report whether a line belongs to the recipe currently being read.

    Returns
    -------
    bool
        True for a tab-indented line, which Make treats as a recipe line.

    Examples
    --------
    >>> is_recipe_line("\techo hi"), is_recipe_line("other:")
    (True, False)
    """
    return line.startswith("\t")


def lading_subcommand(makefile: str, target: str) -> str | None:
    r"""Return the lading subcommand a Make target's recipe invokes.

    The recipe names lading through a variable, so the token is matched by
    suffix: `$(LADING)` expands to a `uvx --from ... lading` invocation.

    Returns
    -------
    str or None
        The first token after the one naming lading, or ``None`` when the
        recipe runs no lading command or names no subcommand.

    Examples
    --------
    >>> lading_subcommand("publish-check:\n\t$(LADING) publish .\n",
    ...                   "publish-check")
    'publish'
    >>> lading_subcommand("publish-check:\n\techo nothing\n",
    ...                   "publish-check") is None
    True
    """
    for line in recipe_lines(makefile, target):
        tokens = shlex.split(line, comments=True)
        named = next(
            (
                index
                for index, token in enumerate(tokens)
                if token == "lading" or token.upper().endswith("LADING)")
            ),
            None,
        )
        if named is None:
            continue
        following = tokens[named + 1 :]
        return following[0] if following else None
    return None


def test_the_preflight_runs_unit_tests_only(
    lading_configuration: dict[str, typ.Any],
) -> None:
    """A local pre-flight must not execute the whole suite again.

    CI skips the pre-flight outright. On a workstation it still runs, and
    this narrows its second stage to the library and binary unit tests so the
    dry run stays usable there rather than being avoided.
    """
    preflight = lading_configuration.get("preflight")
    assert isinstance(preflight, dict), (
        f"lading.toml must declare a [preflight] table, got {preflight!r}"
    )
    assert preflight.get("unit_tests_only") is True, (
        f"lading.toml must set preflight.unit_tests_only = true, got "
        f"{preflight.get('unit_tests_only')!r}; without it a local publish "
        f"check runs the whole workspace suite"
    )


@pytest.mark.parametrize("step_name", TEST_STEPS, ids=str)
def test_every_lane_tests_before_it_packages(
    build_test_job: dict[str, typ.Any], step_name: str
) -> None:
    """Skipping the pre-flight is safe only while this holds.

    The skipped pre-flight relies on these steps having run the workspace. A
    step order that packaged first, or a test step that was removed, would
    leave the job packaging code nothing in it had executed.
    """
    tests_at = step_index(build_test_job, step_name)
    packages_at = step_index(build_test_job, DRY_RUN_STEP)
    assert tests_at < packages_at, (
        f"{step_name!r} runs at index {tests_at}, after the {DRY_RUN_STEP!r} "
        f"step at index {packages_at}; the skipped pre-flight assumes the lane "
        f"has already executed the workspace"
    )


@pytest.mark.parametrize("step_name", TEST_STEPS, ids=str)
def test_every_lane_is_covered_by_both_test_steps(
    build_test_job: dict[str, typ.Any], step_name: str
) -> None:
    """Ordering is not enough on its own: the step must run on that lane.

    A test step that precedes the dry run but whose condition excludes a lane
    leaves that lane packaging without having tested. Both steps are
    unconditional today, which covers the whole matrix; a condition appearing
    here is the change that needs re-reading, so the absence is pinned rather
    than any particular expression.
    """
    declared = step_named(build_test_job, step_name).get("if")
    assert declared is None, (
        f"{step_name!r} is now conditional on {declared!r}; the dry run packages "
        f"on every lane, so a lane this no longer selects would package without "
        f"having tested"
    )


def test_the_skip_is_enabled_on_the_step_that_packages(
    build_test_job: dict[str, typ.Any],
) -> None:
    """CI skips the pre-flight; the value is pinned, not merely present.

    Presence alone proves nothing, because the negative spellings are
    accepted too: `LADING_SKIP_PREFLIGHT: 'false'` sets the variable, passes
    any existence check, and runs the whole pre-flight anyway. That failure is
    silent, which is what makes it worth a contract.
    """
    environment = step_named(build_test_job, DRY_RUN_STEP).get("env") or {}
    assert environment.get(SKIP_VARIABLE) == SKIP_ENABLED, (
        f"the {DRY_RUN_STEP!r} step sets {SKIP_VARIABLE}="
        f"{environment.get(SKIP_VARIABLE)!r}, not {SKIP_ENABLED!r}"
    )


def test_the_skip_is_not_set_for_local_runs(
    lading_configuration: dict[str, typ.Any],
) -> None:
    """A skip in lading.toml would reach a workstation as well.

    On a workstation nothing has run the suite before `make publish-check`,
    so the pre-flight is the only thing checking that the workspace builds and
    its unit tests pass before packaging. The workflow sets the environment
    variable precisely so the two cases can differ.
    """
    preflight = lading_configuration.get("preflight", {})
    assert SKIP_SETTING not in preflight, (
        f"lading.toml sets preflight.{SKIP_SETTING}, which skips the pre-flight "
        f"for local runs too; CI sets {SKIP_VARIABLE} on the publish step instead"
    )


def test_the_publish_step_still_runs_the_packaging_command(
    build_test_job: dict[str, typ.Any],
) -> None:
    """The skip must prune the pre-flight without pruning the packaging.

    `cargo package` builds each crate from its own packaged sources, so it is
    the only thing in CI that sees what a published crate exports. A symbol
    that is public within the workspace but missing from a crate root compiles
    under the workspace test run and Clippy, and fails only here.
    """
    run = str(step_named(build_test_job, DRY_RUN_STEP).get("run", ""))
    assert tuple(shlex.split(run, comments=True)) == PACKAGING_COMMAND, (
        f"the {DRY_RUN_STEP!r} step runs {run.strip()!r}, not "
        f"{' '.join(PACKAGING_COMMAND)!r}; per-crate packaging is what this step "
        f"exists to prove"
    )


def test_the_packaging_target_invokes_lading_publish() -> None:
    """Naming the target is only half of it: the target must package.

    A recipe rewritten to a check-only lading subcommand would leave the
    step's command unchanged and the contract above satisfied, while nothing
    packaged any crate in isolation.
    """
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    assert recipe_lines(makefile, PACKAGING_TARGET), (
        f"the Makefile defines no {PACKAGING_TARGET!r} recipe, so the publish "
        f"step's command packages nothing"
    )
    subcommand = lading_subcommand(makefile, PACKAGING_TARGET)
    assert subcommand == PACKAGING_SUBCOMMAND, (
        f"the {PACKAGING_TARGET!r} target runs `lading {subcommand}`, not "
        f"`lading {PACKAGING_SUBCOMMAND}`; only the publish subcommand builds "
        f"each crate from its own packaged sources"
    )
