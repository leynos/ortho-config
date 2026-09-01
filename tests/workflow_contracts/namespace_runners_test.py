"""Contract-test OrthoConfig's initial Namespace runner assignment."""

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
    assert _job("delayed-pr-comment.yml", "delay_and_comment").get("runs-on") == (
        "namespace-profile-default"
    )


def test_capacity_and_platform_sensitive_matrix_remains_unchanged() -> None:
    """Keep the current Linux and Windows matrix pending equivalent profiles."""
    build_test = _job("ci.yml", "build-test")
    assert build_test.get("runs-on") == "${{ matrix.os }}"
