"""Validate Namespace runner assignments in GitHub Actions workflows."""

from __future__ import annotations

import os
import subprocess
import tempfile
from pathlib import Path

import pytest
import yaml
from hypothesis import given, strategies as st

ROOT = Path(__file__).resolve().parents[2]


def _workflow(workflow_name: str) -> dict[str, object]:
    """Load one repository workflow for structural contract checks."""
    workflow_path = ROOT / ".github" / "workflows" / workflow_name
    workflow = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), f"{workflow_name} must parse to a mapping"
    return workflow


def _job(workflow_name: str, job_name: str) -> dict[str, object]:
    """Load one named job from a repository workflow."""
    workflow = _workflow(workflow_name)
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), f"{workflow_name} must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"{workflow_name} must declare {job_name}"
    return job


def _calculation_step() -> dict[str, object]:
    """Load the delayed-comment calculation step."""
    job = _job("delayed-pr-comment.yml", "delay_and_comment")
    steps = job.get("steps")
    assert isinstance(steps, list), (
        "delayed-pr-comment.yml:delay_and_comment must declare steps"
    )
    step = next(
        (step for step in steps if isinstance(step, dict) and step.get("id") == "calc"),
        None,
    )
    assert isinstance(step, dict), (
        "delayed-pr-comment.yml:delay_and_comment must declare calc step"
    )
    return step


def _run_calculation(delay_minutes: str) -> tuple[int, str, str, str]:
    """Run the workflow calculation and return its process and output results."""
    calculation = _calculation_step().get("run")
    assert isinstance(calculation, str), (
        "delayed-pr-comment.yml:calc must contain executable shell"
    )
    with tempfile.TemporaryDirectory() as temporary_directory:
        output_path = Path(temporary_directory) / "github-output"
        environment = {
            **os.environ,
            "DELAY_MINUTES": delay_minutes,
            "GITHUB_OUTPUT": str(output_path),
        }
        result = subprocess.run(
            ["bash", "-eu"],
            input=calculation,
            text=True,
            capture_output=True,
            env=environment,
            check=False,
        )
        output = output_path.read_text(encoding="utf-8") if output_path.exists() else ""
        return result.returncode, result.stdout, result.stderr, output


def _assert_dispatch_contract(workflow: dict[str, object]) -> None:
    """Assert the delayed-comment workflow's least-privilege inputs."""
    assert workflow.get("permissions") == {}, (
        "delayed-pr-comment.yml must default to no permissions"
    )
    triggers = workflow.get("on", workflow.get(True))
    assert isinstance(triggers, dict), (
        "delayed-pr-comment.yml must declare workflow_dispatch"
    )
    dispatch = triggers.get("workflow_dispatch")
    assert isinstance(dispatch, dict), (
        "delayed-pr-comment.yml must declare workflow_dispatch inputs"
    )
    inputs = dispatch.get("inputs")
    assert isinstance(inputs, dict), (
        "delayed-pr-comment.yml:workflow_dispatch must declare inputs"
    )
    assert {
        name: (definition.get("required"), definition.get("type"))
        for name, definition in inputs.items()
        if isinstance(definition, dict)
    } == {
        "pr_number": (True, "number"),
        "delay_minutes": (True, "number"),
        "message": (True, "string"),
    }, "delayed-pr-comment.yml:workflow_dispatch inputs must retain their types"


def _assert_job_contract(job: dict[str, object]) -> None:
    """Assert the delayed-comment job's runner, timeout, and permissions."""
    assert job.get("runs-on") == ("namespace-profile-default"), (
        "delayed-pr-comment.yml:delay_and_comment must use namespace-profile-default"
    )
    assert job.get("timeout-minutes") == 360, (
        "delayed-pr-comment.yml:delay_and_comment must retain its 360-minute timeout"
    )
    assert job.get("permissions") == {"pull-requests": "write"}, (
        "delayed-pr-comment.yml:delay_and_comment must grant only pull-request write"
    )


def _assert_calculation_contract(calculation: str) -> None:
    """Assert every input guard and decimal conversion in the calc step."""
    assert "*[!0-9]*" in calculation, (
        "delayed-pr-comment.yml:calc must reject non-decimal delay_minutes"
    )
    assert "????*" in calculation, (
        "delayed-pr-comment.yml:calc must reject delay_minutes longer than 3 digits"
    )
    assert "10#$DELAY_MINUTES" in calculation, (
        "delayed-pr-comment.yml:calc must normalise delay_minutes as decimal"
    )
    assert "delay_minutes < 1" in calculation, (
        "delayed-pr-comment.yml:calc must reject delay_minutes below 1"
    )
    assert "delay_minutes > 350" in calculation, (
        "delayed-pr-comment.yml:calc must cap delay_minutes below the job timeout"
    )


def _assert_wait_and_comment_contract(steps: list[object]) -> None:
    """Assert the delayed-comment wait and pinned action wiring."""
    wait_step = next(
        (
            step
            for step in steps
            if isinstance(step, dict) and step.get("name") == "Wait requested time"
        ),
        None,
    )
    assert isinstance(wait_step, dict), (
        "delayed-pr-comment.yml:delay_and_comment must declare wait step"
    )
    assert wait_step.get("shell") == "bash", (
        "delayed-pr-comment.yml:wait must use bash for portable sleep behaviour"
    )
    assert wait_step.get("run") == 'sleep "${{ steps.calc.outputs.secs }}"', (
        "delayed-pr-comment.yml:wait must sleep for the calculated seconds"
    )
    comment_step = next(
        (
            step
            for step in steps
            if isinstance(step, dict) and step.get("name") == "Comment PR"
        ),
        None,
    )
    assert isinstance(comment_step, dict), (
        "delayed-pr-comment.yml:delay_and_comment must declare comment step"
    )
    assert comment_step.get("uses") == (
        "thollander/actions-comment-pull-request@24bffb9b452ba05a4f3f77933840a6a841d1b32b"
    ), "delayed-pr-comment.yml:comment must pin the comment action by full SHA"


def test_comment_job_uses_the_shared_uncached_namespace_profile() -> None:
    """Keep the low-risk utility-job assignment from drifting."""
    workflow = _workflow("delayed-pr-comment.yml")
    _assert_dispatch_contract(workflow)
    job = _job("delayed-pr-comment.yml", "delay_and_comment")
    _assert_job_contract(job)
    steps = job.get("steps")
    assert isinstance(steps, list), (
        "delayed-pr-comment.yml:delay_and_comment must declare steps"
    )
    calculation_step = _calculation_step()
    assert isinstance(calculation_step, dict), (
        "delayed-pr-comment.yml:delay_and_comment must declare the calc step"
    )
    calculation = calculation_step.get("run")
    assert isinstance(calculation, str), (
        "delayed-pr-comment.yml:calc must contain a shell calculation"
    )
    _assert_calculation_contract(calculation)
    _assert_wait_and_comment_contract(steps)


@pytest.mark.parametrize(
    ("delay_minutes", "expected_status", "expected_error", "expected_output"),
    [
        ("1", 0, "", "secs=60\n"),
        ("350", 0, "", "secs=21000\n"),
        ("008", 0, "", "secs=480\n"),
        ("0", 1, "delay_minutes must be between 1 and 350\n", ""),
        ("351", 1, "delay_minutes must be between 1 and 350\n", ""),
        ("", 1, "delay_minutes must be a whole number\n", ""),
        ("abc", 1, "delay_minutes must be a whole number\n", ""),
        ("1000", 1, "delay_minutes must be a whole number\n", ""),
    ],
)
def test_delay_calculation_has_exact_decimal_contract(
    delay_minutes: str,
    expected_status: int,
    expected_error: str,
    expected_output: str,
) -> None:
    """Execute the workflow calculation for accepted and rejected inputs."""
    status, stdout, stderr, actual_output = _run_calculation(delay_minutes)
    assert status == expected_status, (
        f"delayed-pr-comment.yml:calc returned {status} for "
        f"delay_minutes={delay_minutes!r}; stderr={stderr!r}"
    )
    assert stdout == "", (
        f"delayed-pr-comment.yml:calc wrote unexpected stdout for "
        f"delay_minutes={delay_minutes!r}: {stdout!r}"
    )
    assert stderr == expected_error, (
        f"delayed-pr-comment.yml:calc wrote unexpected stderr for "
        f"delay_minutes={delay_minutes!r}: {stderr!r}"
    )
    assert actual_output == expected_output, (
        f"delayed-pr-comment.yml:calc wrote unexpected GITHUB_OUTPUT for "
        f"delay_minutes={delay_minutes!r}: {actual_output!r}"
    )


@given(st.integers(min_value=1, max_value=350))
def test_delay_calculation_preserves_the_valid_range(delay_minutes: int) -> None:
    """Preserve the seconds conversion for every accepted decimal minute."""
    status, stdout, stderr, output = _run_calculation(str(delay_minutes))
    assert status == 0, (
        f"delayed-pr-comment.yml:calc rejected valid delay_minutes={delay_minutes}: "
        f"stderr={stderr!r}"
    )
    assert stdout == "", (
        f"delayed-pr-comment.yml:calc wrote stdout for delay_minutes={delay_minutes}: "
        f"{stdout!r}"
    )
    assert stderr == "", (
        f"delayed-pr-comment.yml:calc wrote stderr for delay_minutes={delay_minutes}: "
        f"{stderr!r}"
    )
    assert output == f"secs={delay_minutes * 60}\n", (
        f"delayed-pr-comment.yml:calc emitted the wrong seconds for "
        f"delay_minutes={delay_minutes}: {output!r}"
    )


def test_capacity_and_platform_sensitive_matrix_remains_unchanged() -> None:
    """Keep the current Linux and Windows matrix pending equivalent profiles."""
    build_test = _job("ci.yml", "build-test")
    assert build_test.get("runs-on") == "${{ matrix.os }}", (
        "ci.yml:build-test must resolve its runner from matrix.os"
    )
    strategy = build_test.get("strategy")
    assert isinstance(strategy, dict), "ci.yml:build-test must declare strategy"
    matrix = strategy.get("matrix")
    assert isinstance(matrix, dict), "ci.yml:build-test must declare a matrix"
    include = matrix.get("include")
    assert isinstance(include, list), "ci.yml:build-test matrix must declare include"
    matrix_oses = {entry.get("os") for entry in include if isinstance(entry, dict)}
    assert {"ubuntu-latest", "windows-latest"} <= matrix_oses, (
        "ci.yml:build-test matrix must retain ubuntu-latest and windows-latest"
    )
