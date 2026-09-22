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

import re
import shlex
import tomllib
import typing as typ
from pathlib import Path

import pytest
import yaml
from makefile_support import lading_subcommand, recipe_lines

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


def _steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return a job's steps."""
    return list(job.get("steps") or [])


def _step_named(job: dict[str, typ.Any], name: str) -> dict[str, typ.Any]:
    """Return the one step called *name*, failing when it is absent."""
    matches = [step for step in _steps(job) if step.get("name") == name]
    assert len(matches) == 1, (
        f"expected exactly one step named {name!r} in {BUILD_TEST_JOB!r}, "
        f"found {len(matches)}"
    )
    return matches[0]


def _step_index(job: dict[str, typ.Any], name: str) -> int:
    """Return the position of the step called *name*."""
    for index, step in enumerate(_steps(job)):
        if step.get("name") == name:
            return index
    message = f"{BUILD_TEST_JOB!r} declares no step named {name!r}"
    raise AssertionError(message)


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
    tests_at = _step_index(build_test_job, step_name)
    packages_at = _step_index(build_test_job, DRY_RUN_STEP)
    assert tests_at < packages_at, (
        f"{step_name!r} runs at index {tests_at}, after the {DRY_RUN_STEP!r} "
        f"step at index {packages_at}; the skipped pre-flight assumes the lane "
        f"has already executed the workspace"
    )


@pytest.mark.parametrize("step_name", TEST_STEPS, ids=str)
def test_a_failing_test_step_stops_the_lane(
    build_test_job: dict[str, typ.Any], step_name: str
) -> None:
    """A test that may fail without stopping the job tests nothing here.

    `continue-on-error: true` lets the job carry on to the dry run after
    the step has failed, and the skip means lading will not rerun it: the
    pre-flight that would have caught the failure is exactly what this
    branch removes. The lane would then package code whose tests failed
    and report success.

    Absence is asserted as well as the false value, because the input is
    optional and its default is what the lane relies on. `safe_load`
    resolves the YAML boolean, so `false` arrives as `False` rather than
    as a string; an expression would arrive as text and is refused for
    the same reason a condition is refused above, namely that whether it
    stops the lane could then depend on the run.
    """
    declared = _step_named(build_test_job, step_name).get("continue-on-error")
    assert declared in (None, False), (
        f"{step_name!r} sets continue-on-error to {declared!r}; a failure "
        f"there would reach the {DRY_RUN_STEP!r} step, and the skipped "
        f"pre-flight will not rerun it, so the lane would package code whose "
        f"tests failed"
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
    declared = _step_named(build_test_job, step_name).get("if")
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
    environment = _step_named(build_test_job, DRY_RUN_STEP).get("env") or {}
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
    run = str(_step_named(build_test_job, DRY_RUN_STEP).get("run", ""))
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


#: The variable the Makefile must resolve lading through, and the shape its
#: value must take. A full commit SHA, because a tag can be repointed and an
#: absent pin tracks lading's default branch: either way the release gate
#: would change what it runs with no edit to this repository. The value
#: itself is not asserted, so bumping the pin stays a one-line change.
LADING_PIN_VARIABLE: typ.Final[str] = "LADING_REF"
LADING_PIN_RE: typ.Final = re.compile(
    rf"^{LADING_PIN_VARIABLE}\s*\?=\s*([0-9a-f]{{40}})\s*$", re.MULTILINE
)

#: The `uvx --from` spec the lading command must use, with the pin
#: interpolated rather than a bare repository URL.
LADING_SPEC_RE: typ.Final = re.compile(
    rf"^LADING\s*\?=.*git\+https://github\.com/leynos/lading@\$\("
    rf"{LADING_PIN_VARIABLE}\).*$",
    re.MULTILINE,
)

#: The flag that makes the contract helpers' own examples run.
DOCTEST_FLAG: typ.Final[str] = "--doctest-modules"
CONTRACT_TARGET: typ.Final[str] = "test-workflow-contracts"


def test_lading_is_pinned_to_a_commit() -> None:
    """An unpinned lading changes the release gate with no edit here.

    Before this was pinned, `uvx --from git+...` resolved lading's default
    branch on every run, so what the gate did depended on when it ran. The
    SHA's shape is asserted rather than its value, so bumping the pin does not
    become a two-file chore.
    """
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    assert LADING_PIN_RE.search(makefile), (
        f"the Makefile sets no {LADING_PIN_VARIABLE} to a full 40-hex commit "
        f"SHA; without one the publish gate follows lading's default branch"
    )


def test_the_lading_command_uses_the_pin() -> None:
    """Declaring the pin is not enough: the command must interpolate it.

    A `LADING_REF` nothing reads is a comment. The companion to the test
    above, because either half alone leaves the gate unpinned in practice.
    """
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    assert LADING_SPEC_RE.search(makefile), (
        f"the LADING command does not resolve lading at "
        f"$({LADING_PIN_VARIABLE}); the pin above would then be unused"
    )


def test_the_contract_gate_runs_its_own_examples() -> None:
    """The helpers here carry examples, so they must be executed.

    They document how a Makefile recipe is parsed. An example that has
    drifted from the parser is worse than none, because it reads as verified.
    """
    makefile = MAKEFILE_PATH.read_text(encoding="utf-8")
    recipe = " ".join(recipe_lines(makefile, CONTRACT_TARGET))
    assert recipe, f"the Makefile defines no {CONTRACT_TARGET!r} recipe"
    assert DOCTEST_FLAG in recipe, (
        f"the {CONTRACT_TARGET!r} recipe does not pass {DOCTEST_FLAG}, so the "
        f"examples in these helpers are displayed rather than run"
    )
