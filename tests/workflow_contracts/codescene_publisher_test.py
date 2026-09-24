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

#: The publisher's concurrency group, exactly (CV-005 sweep item 6).
PUBLISHER_GROUP: typ.Final[str] = "coverage-main-${{ github.ref }}"


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
    assert concurrency.get("group") == PUBLISHER_GROUP, (
        f"{name}'s concurrency group must be exactly {PUBLISHER_GROUP!r}, keyed "
        f"on the ref alone: a static group lets a dispatch on another branch "
        f"displace a pending main run, and one keyed on the event lets an "
        f"older run upload last. It declares {concurrency.get('group')!r}"
    )
    cancels = str(concurrency.get("cancel-in-progress", "false")).strip()
    assert cancels == "false", (
        f"{name} must queue superseded publisher runs rather than cancel "
        f"them; a cancelled run abandons its upload and its baseline "
        f"write. It declares cancel-in-progress {cancels!r}"
    )


#: The token check's sole command, exactly. The expression is evaluated
#: before the shell runs, so the step writes ``true`` or ``false`` and no
#: process ever holds the token.
TOKEN_CHECK_COMMAND: typ.Final[str] = (
    "echo \"available=${{ secrets.CS_ACCESS_TOKEN != '' }}\" >> \"$GITHUB_OUTPUT\""
)

#: How the upload receives the token: as the action's input, never as env.
TOKEN_INPUT: typ.Final[str] = "${{ secrets.CS_ACCESS_TOKEN }}"


def _publisher_job(
    documents: dict[str, WorkflowDocument],
) -> tuple[str, list[dict[str, object]]]:
    """Return the publisher's name and the steps of the job that uploads."""
    ((name, document),) = publishers(documents).items()
    jobs = document.get("jobs")
    assert isinstance(jobs, dict), f"{name} declares no jobs mapping"
    for job in jobs.values():
        steps = job.get("steps") if isinstance(job, dict) else None
        if isinstance(steps, list) and any(
            isinstance(step, dict) and CODESCENE_ACTION in str(step.get("uses", ""))
            for step in steps
        ):
            return name, [step for step in steps if isinstance(step, dict)]
    pytest.fail(f"no job in {name} invokes {CODESCENE_ACTION}")


def _token_check(steps: list[dict[str, object]]) -> tuple[int, dict[str, object]]:
    """Return the index and step whose sole command is the token check."""
    found = [
        (index, step)
        for index, step in enumerate(steps)
        if str(step.get("run", "")).strip() == TOKEN_CHECK_COMMAND
    ]
    assert len(found) == 1, (
        f"the upload's job must run `{TOKEN_CHECK_COMMAND}` as one step's "
        f"sole command; {len(found)} steps do"
    )
    return found[0]


def test_the_token_check_is_one_exact_unguarded_command(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The upload's precondition is computed where no process sees the token.

    A guard reading ``env.CS_ACCESS_TOKEN`` passes with the binding deleted,
    after which the upload skips forever, so the check is asserted
    positively: one step, ahead of the upload in its job, with an ``id``,
    no ``if:`` (``false && X`` contains X) and no ``env``, whose sole
    command is the exact echo.
    """
    name, steps = _publisher_job(documents)
    index, check = _token_check(steps)
    upload = next(
        position
        for position, step in enumerate(steps)
        if CODESCENE_ACTION in str(step.get("uses", ""))
    )
    assert index < upload, f"{name} checks the token after the upload runs"
    assert check.get("id"), f"{name}'s token check has no id to read it by"
    assert "if" not in check, f"{name}'s token check is guarded: {check.get('if')!r}"
    assert "env" not in check, f"{name}'s token check binds env: {check.get('env')!r}"


def test_the_upload_consumes_the_check_and_the_ref(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The guard is exactly the check's output and the main ref.

    Held as a set, so an extra ``false`` conjunct fails as surely as a
    missing one; ``||`` is refused by ``test_the_publisher_uploads_only_from_main``.
    """
    _, steps = _publisher_job(documents)
    _, check = _token_check(steps)
    expected = {f"steps.{check.get('id')}.outputs.available == 'true'", MAIN_REF}
    condition = str(_publisher_upload(documents).get("if", ""))
    assert set(_conjuncts(condition)) == expected, (
        f"the upload must be guarded on exactly {sorted(expected)}; it is "
        f"guarded on {condition!r}"
    )


def test_the_upload_takes_the_token_as_its_input(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The action binds the token itself from ``access-token``."""
    inputs = _publisher_upload(documents).get("with") or {}
    assert isinstance(inputs, dict), f"the upload step's `with:` is {inputs!r}"
    assert inputs.get("access-token") == TOKEN_INPUT, (
        f"the upload must pass `access-token: {TOKEN_INPUT}`; it passes "
        f"{inputs.get('access-token')!r}"
    )


def _env_blocks(document: WorkflowDocument) -> list[object]:
    """Return every ``env`` mapping in a workflow: top level, job and step."""
    blocks: list[object] = [document.get("env")]
    jobs = document.get("jobs")
    for job in jobs.values() if isinstance(jobs, dict) else []:
        if isinstance(job, dict):
            blocks.append(job.get("env"))
    blocks.extend(step.get("env") for step in workflow_steps(document))
    return [block for block in blocks if block is not None]


def test_no_env_in_the_publisher_carries_the_token(
    documents: dict[str, WorkflowDocument],
) -> None:
    """The composite upload passes its step env to nested steps.

    So the token is refused in every ``env`` block of the publisher, at
    workflow, job and step level alike.
    """
    ((name, document),) = publishers(documents).items()
    carrying = [
        block for block in _env_blocks(document) if "CS_ACCESS_TOKEN" in str(block)
    ]
    assert not carrying, f"{name} binds the token in env: {carrying}"
