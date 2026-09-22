"""What the single CodeScene publisher must look like.

Separated from ``codescene_coverage_test``, whose subject is which
workflow may upload at all, so neither module outgrows the 400-line
limit ``AGENTS.md`` sets. The subject here is the one that may: what
its upload step is guarded on, what it passes, and what stops two of
its runs racing.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest
from codescene_coverage import CODESCENE_ACTION, publishers
from workflow_reading import read_workflows, workflow_steps

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from workflow_reading import WorkflowDocument

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".github" / "workflows"

#: The ref the publisher's upload step must be guarded on.
MAIN_REF: typ.Final[str] = "github.ref == 'refs/heads/main'"


@pytest.fixture(scope="module")
def documents() -> dict[str, WorkflowDocument]:
    """Return this repository's workflows, parsed once.

    Returns
    -------
    dict
        File name to parsed document.
    """
    return read_workflows(WORKFLOWS)


def _publisher_upload(documents: dict[str, WorkflowDocument]) -> dict[str, object]:
    """Return the publisher's single CodeScene upload step."""
    ((name, document),) = publishers(documents).items()
    steps = [
        step
        for step in workflow_steps(document)
        if CODESCENE_ACTION in str(step.get("uses", ""))
    ]
    assert len(steps) == 1, (
        f"{name} must invoke {CODESCENE_ACTION} once; it invokes it "
        f"{len(steps)} times"
    )
    return steps[0]


def test_the_publisher_uploads_rather_than_checks(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The mode is named, not left to the action's default.

    ``mode`` decides whether the step uploads a report or gates a pull
    request against one, and the default has changed before. Naming it
    is how a reader of these lines knows which of the two this step does
    without reading the action.
    """
    inputs = _publisher_upload(documents).get("with") or {}
    assert isinstance(inputs, dict), f"the upload step's `with:` is {inputs!r}"
    assert inputs.get("mode") == "upload", (
        f"the publisher's CodeScene step must name `mode: upload`; it names "
        f"{inputs.get('mode')!r}"
    )


def test_the_publisher_passes_no_deprecated_checksum(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The old checksum input fails the run outright.

    ``installer-checksum`` is rejected when non-empty from this pin, and
    ``archive-checksum`` is not a rename of it: it digests the action's
    CLI manifest archive, while the repository variable the old input
    carried holds the installer script's digest. Carrying the value
    across under the new name would fail every run, so neither input is
    passed and the action's own manifest pins the CLI instead.
    """
    inputs = _publisher_upload(documents).get("with") or {}
    assert isinstance(inputs, dict), f"the upload step's `with:` is {inputs!r}"
    for rejected in ("installer-checksum", "archive-checksum"):
        assert rejected not in inputs, (
            f"the publisher passes {rejected!r}; `installer-checksum` is "
            f"rejected when non-empty and `archive-checksum` digests a "
            f"different artefact from the variable this repository holds"
        )


def _conjuncts(condition: str) -> list[str]:
    """Return an ``if:`` condition's ``&&`` terms, whitespace-normalized."""
    body = condition.strip()
    if body.startswith("${{") and body.endswith("}}"):
        body = body[3:-2]
    return [" ".join(term.split()) for term in body.split("&&")]


def test_the_publisher_uploads_only_from_main(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The token is not enough; the ref has to be checked too.

    The publisher declares no ``workflow_dispatch`` today, so this guard
    changes nothing now. It is asserted because a dispatch can be aimed
    at any branch and the upload carries no ref, so adding one later
    would otherwise let a run from a feature branch publish that
    branch's coverage as the trunk's, silently, and every pull request
    would then ratchet against it.

    The guard must be a conjunct, not merely present. Finding the ref
    comparison as a substring passes ``... && ref == main || dispatch``,
    in which ``&&`` binds tighter and every conjunct becomes optional,
    so a dispatch from any branch uploads. An ``||`` anywhere is refused
    rather than parsed: no correct guard here needs one.
    """
    condition = str(_publisher_upload(documents).get("if", ""))
    assert "||" not in condition, (
        f"the publisher's upload guard contains `||`, which makes the ref "
        f"check optional: {condition!r}"
    )
    assert MAIN_REF in _conjuncts(condition), (
        f"the publisher's upload step must be guarded on {MAIN_REF} as one "
        f"`&&` term; it is guarded on {condition!r}"
    )


def test_the_publisher_runs_one_at_a_time(
    documents: dict[str, WorkflowDocument],
) -> None:
    """Publisher runs queue; a superseded one is never cancelled.

    Two runs racing would decide the baseline by finishing order, so the
    workflow names a concurrency group. Cancelling within it is the
    wrong remedy: a cancelled run abandons both its upload and its
    ratchet baseline write, while a queued one publishes later and the
    later push's baseline still wins, because it runs last. The
    pull-request lanes cancel superseded runs; the publisher must not.
    """
    ((name, document),) = publishers(documents).items()
    concurrency = document.get("concurrency")
    assert isinstance(concurrency, dict), (
        f"{name} must declare a concurrency block; two publisher runs "
        f"otherwise race on the ratchet baseline. It declares {concurrency!r}"
    )
    assert concurrency.get("group"), (
        f"{name}'s concurrency block must name a group: {concurrency!r}"
    )
    cancels = str(concurrency.get("cancel-in-progress", "false")).strip()
    assert cancels == "false", (
        f"{name} must queue superseded publisher runs rather than cancel "
        f"them; a cancelled run abandons its upload and its baseline "
        f"write. It declares cancel-in-progress {cancels!r}"
    )
