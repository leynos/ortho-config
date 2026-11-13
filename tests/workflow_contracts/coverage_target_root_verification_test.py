"""Contracts for the steps that verify the coverage target root in use.

`coverage_command_line_test.py` pins the root each platform chooses: the
drive-root directory on Windows and the `RUNNER_TEMP` directory on Linux, both
published to the coverage actions through `GITHUB_ENV`. That records what the
workflow published, not what the coverage processes received. The
33,497-character command line behind issue #612 was refused before `rustc`
ran, so nothing in the Rust tree could report it and no local gate can observe
it.

Two steps therefore read the environment back after both coverage passes and
require the directory the passes built in. This module pins their guards,
shells, positions and assertions; `coverage_target_root_behaviour_test.py`
executes the same scripts against supplied environments.

Run via `make test-workflow-contracts`.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest

from workflow_reading import read_workflows, workflow_jobs

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"

BUILD_TEST_JOB: typ.Final[str] = "build-test"
WINDOWS_ONLY: typ.Final[str] = "${{ matrix.platform == 'windows' }}"
LINUX_ONLY: typ.Final[str] = "${{ matrix.platform == 'linux' }}"

WINDOWS_VERIFY_STEP: typ.Final[str] = "Verify the Windows coverage target root"
LINUX_VERIFY_STEP: typ.Final[str] = "Verify the Linux coverage target root"

#: The coverage actions whose observed environment the verification reads.
COVERAGE_STEPS: typ.Final[tuple[str, ...]] = (
    "Test and Measure Coverage (with serde_saphyr)",
    "Test and Measure Coverage (without serde_saphyr)",
)

#: The variables each chooser publishes, and each verification reads back.
COVERAGE_VARIABLES: typ.Final[tuple[str, ...]] = (
    "CARGO_TARGET_DIR",
    "CARGO_LLVM_COV_TARGET_DIR",
)

#: The directory name both choosers select, and both checks require.
DERIVED_DIRECTORY: typ.Final[str] = "llvm-cov-target"

#: The summary line each successful check writes, so a passing run says which
#: root it observed rather than only that it observed one.
OBSERVED_LINE: typ.Final[str] = "Both coverage passes built under"


def _build_test_steps() -> list[dict[str, typ.Any]]:
    """Return ``build-test``'s steps, in declaration order."""
    job = workflow_jobs(read_workflows(WORKFLOWS)["ci.yml"])[BUILD_TEST_JOB]
    return list(job.get("steps") or [])


def _one_step(steps: list[dict[str, typ.Any]], name: str) -> dict[str, typ.Any]:
    """Return the single step named ``name``, failing clearly otherwise."""
    found = [step for step in steps if step.get("name") == name]
    assert len(found) == 1, f"{BUILD_TEST_JOB} has {len(found)} steps named {name!r}"
    return found[0]


def _index_of(steps: list[dict[str, typ.Any]], name: str) -> int:
    """Return the position of the single step named ``name``."""
    _one_step(steps, name)
    return [step.get("name") for step in steps].index(name)


def _condition(step: dict[str, typ.Any]) -> str:
    """Return a step's ``if`` with whitespace normalised."""
    return " ".join(str(step.get("if", "")).split())


def _run(step: dict[str, typ.Any]) -> str:
    """Return a step's run block."""
    return str(step.get("run", ""))


def _environment_blocks(
    document: dict[typ.Union[str, bool], object],
) -> typ.Iterator[tuple[str, dict[str, typ.Any]]]:
    """Yield every ``env`` mapping with a label naming where it was declared.

    >>> document = {
    ...     "env": {"A": "1"},
    ...     "jobs": {"j": {"env": {"B": "2"}, "steps": [{"env": {"C": "3"}}]}},
    ... }
    >>> [label for label, _ in _environment_blocks(document)]
    ['workflow env', 'job j env', 'step 0 env']
    """
    workflow_environment = document.get("env")
    if isinstance(workflow_environment, dict):
        yield "workflow env", workflow_environment
    for job_name, job in workflow_jobs(document).items():
        environment = job.get("env")
        if isinstance(environment, dict):
            yield f"job {job_name} env", environment
        for index, step in enumerate(job.get("steps") or []):
            environment = step.get("env")
            if isinstance(environment, dict):
                yield f"step {index} env", environment


@pytest.mark.parametrize(
    ("name", "condition", "shell"),
    [
        pytest.param(WINDOWS_VERIFY_STEP, WINDOWS_ONLY, "pwsh", id="windows"),
        pytest.param(LINUX_VERIFY_STEP, LINUX_ONLY, "bash", id="linux"),
    ],
)
def test_each_verification_runs_on_one_platform_in_its_own_shell(
    name: str, condition: str, shell: str
) -> None:
    """Each check is scoped to the leg whose root it reads and runs its shell."""
    step = _one_step(_build_test_steps(), name)
    assert _condition(step) == condition, (
        f"{name!r} is conditioned on {_condition(step)!r}, not {condition!r}"
    )
    assert step.get("shell") == shell, (
        f"{name!r} runs in {step.get('shell')!r}, not {shell!r}"
    )


@pytest.mark.parametrize("name", [WINDOWS_VERIFY_STEP, LINUX_VERIFY_STEP])
def test_each_verification_follows_both_coverage_passes(name: str) -> None:
    """A check placed before a pass would verify an environment it has not seen."""
    steps = _build_test_steps()
    verification = _index_of(steps, name)
    for coverage in COVERAGE_STEPS:
        assert _index_of(steps, coverage) < verification, (
            f"{name!r} appears before {coverage!r}, so a pass could still run "
            "under a root the check never observed"
        )


@pytest.mark.parametrize("variable", COVERAGE_VARIABLES)
def test_the_windows_check_rejects_either_variable_going_astray(variable: str) -> None:
    """Both variables carry the history, so both are read back on Windows."""
    run = _run(_one_step(_build_test_steps(), WINDOWS_VERIFY_STEP))
    assert f"$env:{variable}" in run, (
        f"the Windows check never reads {variable}, so a pass could build "
        "under a root the check does not look at"
    )
    assert f"{variable} is '" in run, (
        f"the Windows check must name {variable} in its failure message"
    )


@pytest.mark.parametrize("variable", COVERAGE_VARIABLES)
def test_the_linux_check_rejects_either_variable_going_astray(variable: str) -> None:
    """The Linux check reads the same two variables back."""
    run = _run(_one_step(_build_test_steps(), LINUX_VERIFY_STEP))
    assert f"${{{variable}-}}" in run, (
        f"the Linux check never reads {variable}, so a pass could build "
        "under a root the check does not look at"
    )
    assert f"{variable} is '" in run, (
        f"the Linux check must name {variable} in its failure message"
    )


def test_the_windows_check_derives_the_same_root_as_the_chooser() -> None:
    """The check recomputes the chooser's drive-root directory, not a copy."""
    steps = _build_test_steps()
    chooser = _run(_one_step(steps, "Choose the coverage target root"))
    check = _run(_one_step(steps, WINDOWS_VERIFY_STEP))

    for expression in (
        "[System.IO.Path]::GetPathRoot($env:RUNNER_TEMP)",
        f"'{DERIVED_DIRECTORY}'",
    ):
        assert expression in chooser, (
            f"the chooser no longer derives its root with {expression!r}, so "
            "the check beside it compares against the wrong directory"
        )
        assert expression in check, (
            f"the Windows check does not derive its expected root with "
            f"{expression!r}, so it is not observing the chooser's decision"
        )


def test_the_windows_check_requires_the_derived_directory_to_exist() -> None:
    """Matching variables are not enough: the passes must have built there."""
    run = _run(_one_step(_build_test_steps(), WINDOWS_VERIFY_STEP))
    assert "Test-Path -LiteralPath $expected" in run, (
        "the Windows check must observe the directory the builds produced"
    )
    assert "-not (Test-Path -LiteralPath $expected)" in run, (
        "an absent directory must fail the lane, not merely be reported"
    )
    assert "is missing" in run, "a missing directory must be named in the failure"


def test_the_linux_check_requires_the_chosen_directory_to_exist() -> None:
    """The nested trybuild projects' target must be on disk after both passes."""
    steps = _build_test_steps()
    chooser = _run(_one_step(steps, "Choose the Linux coverage target root"))
    check = _run(_one_step(steps, LINUX_VERIFY_STEP))

    assert '"$RUNNER_TEMP/llvm-cov-target"' in chooser, (
        "the Linux chooser no longer selects the root the check expects"
    )
    assert 'expected="$RUNNER_TEMP/llvm-cov-target"' in check, (
        "the Linux check must recompute the chooser's root"
    )
    assert '[ ! -d "$expected" ]' in check, (
        "the Linux check must observe the directory the builds produced"
    )
    assert "is missing" in check, "a missing directory must be named in the failure"


@pytest.mark.parametrize("name", [WINDOWS_VERIFY_STEP, LINUX_VERIFY_STEP])
def test_each_verification_reports_the_root_it_observed(name: str) -> None:
    """A passing run says which root was in use, not only that it passed."""
    run = _run(_one_step(_build_test_steps(), name))
    assert OBSERVED_LINE in run, (
        f"{name!r} must report the root it observed"
    )
    assert "GITHUB_STEP_SUMMARY" in run, (
        f"{name!r} must publish the observed root to the job summary"
    )


def test_no_workflow_block_declares_a_coverage_variable() -> None:
    """No ``env`` block may shadow what the choosers publish.

    ``GITHUB_ENV`` writes are merged into the environment for later steps; an
    ``env`` declaration at workflow, job or step level would give a consumer a
    value the check does not inspect, and the check would then verify a
    variable nothing built under.
    """
    document = read_workflows(WORKFLOWS)["ci.yml"]
    declared = list(_environment_blocks(document))
    assert declared, "the workflow has no env blocks at all; the reader is broken"
    for label, mapping in declared:
        for variable in COVERAGE_VARIABLES:
            assert variable not in mapping, (
                f"{label} declares {variable}, which shadows the chooser's "
                "value and leaves the verification reading a different variable"
            )