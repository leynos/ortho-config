"""Validate Namespace runner assignments in GitHub Actions workflows."""

from __future__ import annotations

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


def _job(workflow_name: str, job_name: str) -> dict[str, object]:
    """Load one named job from a repository workflow."""
    workflow_path = ROOT / ".github" / "workflows" / workflow_name
    workflow = yaml.safe_load(workflow_path.read_text(encoding="utf-8"))
    assert isinstance(workflow, dict), f"{workflow_name} must parse to a mapping"
    jobs = workflow.get("jobs")
    assert isinstance(jobs, dict), f"{workflow_name} must declare jobs"
    job = jobs.get(job_name)
    assert isinstance(job, dict), f"{workflow_name} must declare {job_name}"
    return job


def test_comment_job_uses_the_shared_uncached_namespace_profile() -> None:
    """Keep the low-risk utility-job assignment from drifting."""
    job = _job("delayed-pr-comment.yml", "delay_and_comment")
    assert job.get("runs-on") == (
        "namespace-profile-default"
    ), "delayed-pr-comment.yml:delay_and_comment must use namespace-profile-default"
    assert job.get("timeout-minutes") == 360, (
        "delayed-pr-comment.yml:delay_and_comment must retain its 360-minute timeout"
    )
    steps = job.get("steps")
    assert isinstance(steps, list), (
        "delayed-pr-comment.yml:delay_and_comment must declare steps"
    )
    calculation_step = next(
        (step for step in steps if isinstance(step, dict) and step.get("id") == "calc"),
        None,
    )
    assert isinstance(calculation_step, dict), (
        "delayed-pr-comment.yml:delay_and_comment must declare the calc step"
    )
    calculation = calculation_step.get("run")
    assert isinstance(calculation, str) and "DELAY_MINUTES > 350" in calculation, (
        "delayed-pr-comment.yml:calc must cap delay_minutes below the job timeout"
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
