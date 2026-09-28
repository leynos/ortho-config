"""Prove the `codescene` environment sits on the uploading job alone.

Each test mutates a copy of this repository's workflows the way a later edit
could, and asserts the clause meant to catch it does. The check step, the ref
guard and `access-token:` stay held by the publisher contract.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import copy
import pathlib
import typing as typ

import pytest
from codescene_coverage import CODESCENE_ACTION
from codescene_environment import (
    MISSING,
    NO_UPLOADER,
    REACHABLE,
    STRAY,
    UNRESOLVED,
    environment_violations,
    pull_request_closure,
)
from workflow_reading import (
    WorkflowReadingError,
    load_workflow,
    read_workflows,
    workflow_jobs,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from workflow_reading import WorkflowDocument

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"
PUBLISHER: typ.Final[str] = "coverage-main.yml"
LANE: typ.Final[str] = "ci.yml"
CALLEE_JOB: typ.Final[str] = "  inner:\n    environment: codescene\n    runs-on: x\n"


@pytest.fixture
def documents() -> dict[str, WorkflowDocument]:
    """Return a private copy of the repository's workflows to mutate.

    Returns
    -------
    dict
        The parsed workflows, deep-copied for this test alone.
    """
    return copy.deepcopy(read_workflows(WORKFLOWS))


def _first_job(documents: dict[str, WorkflowDocument], name: str) -> dict[str, object]:
    """Return one workflow's first job, for mutation in place."""
    return next(iter(workflow_jobs(documents[name]).values()))


def _add_job(
    documents: dict[str, WorkflowDocument], name: str, job_id: str, job: object
) -> None:
    """Add one job to a workflow, for mutation in place."""
    typ.cast("dict[str, object]", documents[name]["jobs"])[job_id] = job


def _reports(documents: dict[str, WorkflowDocument], fragment: str) -> None:
    """Fail unless the rule reports a violation containing `fragment`."""
    found = environment_violations(documents)
    assert any(fragment in problem for problem in found), (
        f"expected a violation naming {fragment!r}, got {found}"
    )


def test_repository_places_the_environment(documents: dict[str, WorkflowDocument]) -> None:
    """The publisher declares the environment and nothing else does."""
    found = environment_violations(documents)
    assert not found, f"expected no violations, got {found}"


def test_the_pull_request_lanes_are_read(documents: dict[str, WorkflowDocument]) -> None:
    """The closure reaches both pull-request lanes and not the publisher."""
    reached = pull_request_closure(documents)
    assert {LANE, "dependabot-automerge.yml"} <= reached, reached
    assert PUBLISHER not in reached, f"{PUBLISHER} must not be pull-request reachable"


def test_publisher_cannot_drop_the_environment(documents: dict[str, WorkflowDocument]) -> None:
    """Without it the moved token never reaches the upload, which then skips."""
    del _first_job(documents, PUBLISHER)["environment"]
    _reports(documents, MISSING)


def test_publisher_cannot_name_another_environment(
    documents: dict[str, WorkflowDocument],
) -> None:
    """Another environment holds no CodeScene token."""
    _first_job(documents, PUBLISHER)["environment"] = "production"
    _reports(documents, MISSING)


def test_mapping_form_is_accepted(documents: dict[str, WorkflowDocument]) -> None:
    """`{name: codescene}` is the same declaration as the bare string."""
    _first_job(documents, PUBLISHER)["environment"] = {"name": "codescene"}
    found = environment_violations(documents)
    assert not found, f"the mapping form must be accepted, got {found}"


@pytest.mark.parametrize("name", ["codescene", "CodeScene"])
def test_no_other_job_may_declare_it(
    documents: dict[str, WorkflowDocument], name: str
) -> None:
    """A second holder of the token widens what can read it, in any case."""
    _add_job(documents, PUBLISHER, "other", {"environment": name, "steps": [{"run": "true"}]})
    _reports(documents, f"{PUBLISHER}:other {STRAY}")


def test_a_look_alike_action_is_not_the_uploader(
    documents: dict[str, WorkflowDocument],
) -> None:
    """Only the shared uploader's exact path earns the environment."""
    look_alike = {"uses": f"{CODESCENE_ACTION}-check@v1"}
    _add_job(documents, PUBLISHER, "other", {"environment": "codescene", "steps": [look_alike]})
    _reports(documents, f"{PUBLISHER}:other {STRAY}")


def test_an_expression_named_environment_is_refused(
    documents: dict[str, WorkflowDocument],
) -> None:
    """A computed name may resolve to `codescene`, so it cannot be placed."""
    _add_job(documents, LANE, "computed", {"environment": {"name": "${{ 'codescene' }}"}})
    _reports(documents, f"{LANE}:computed {UNRESOLVED}")


@pytest.mark.parametrize("name", ["codescene", "CodeScene"])
def test_no_pull_request_job_may_declare_it(
    documents: dict[str, WorkflowDocument], name: str
) -> None:
    """A pull request's own code must never be able to request the token."""
    _first_job(documents, LANE)["environment"] = {"name": name}
    first = next(iter(workflow_jobs(documents[LANE])))
    _reports(documents, f"{LANE}:{first} {REACHABLE}")


@pytest.mark.parametrize("prefix", ["./", "$/"])
def test_a_called_workflow_is_read_too(
    documents: dict[str, WorkflowDocument], prefix: str
) -> None:
    """A workflow the lane calls runs for the pull request as well."""
    documents["called.yml"] = load_workflow(f"on:\n  workflow_call:\njobs:\n{CALLEE_JOB}")
    _add_job(documents, LANE, "forward", {"uses": f"{prefix}.github/workflows/called.yml"})
    _reports(documents, f"called.yml:inner {REACHABLE}")


@pytest.mark.parametrize(
    "trigger",
    [
        "workflow_run:\n    workflows: [CI]",
        "merge_group:",
        "issue_comment:",
        "push:\n    branches: [wip]",
        "push:",
    ],
)
def test_other_pull_request_routes_are_read(
    documents: dict[str, WorkflowDocument], trigger: str
) -> None:
    """Chains, queues, comments and branch pushes all run branch-reachable code."""
    documents["after.yml"] = load_workflow(f"on:\n  {trigger}\njobs:\n{CALLEE_JOB}")
    _reports(documents, f"after.yml:inner {REACHABLE}")


@pytest.mark.parametrize("trigger", ["push:\n    branches: [main]", "push:\n    tags: ['v*']"])
def test_main_and_tag_pushes_are_not_pull_request_routes(
    documents: dict[str, WorkflowDocument], trigger: str
) -> None:
    """The closure stays narrow: a trunk or tag push is not branch code."""
    documents["after.yml"] = load_workflow(f"on:\n  {trigger}\njobs:\n  inner:\n    runs-on: x\n")
    assert "after.yml" not in pull_request_closure(documents)


def test_a_tree_with_no_pull_request_route_is_refused(
    documents: dict[str, WorkflowDocument],
) -> None:
    """A closure with no seed is a broken reader, not a compliant repository."""
    with pytest.raises(WorkflowReadingError, match="no workflow serves a pull request"):
        pull_request_closure({PUBLISHER: documents[PUBLISHER]})


def test_an_empty_reading_is_refused(documents: dict[str, WorkflowDocument]) -> None:
    """With no uploader left the rule says so rather than passing."""
    job = _first_job(documents, PUBLISHER)
    job["steps"] = [
        step
        for step in typ.cast("list[dict[str, object]]", job["steps"])
        if CODESCENE_ACTION not in str(step.get("uses", ""))
    ]
    _reports(documents, NO_UPLOADER)
