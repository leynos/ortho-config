"""Hold the CodeScene token's environment to the uploading job (CV-005).

The token belongs in the `codescene` environment, whose deployment policy
admits `main` alone; until the owner moves it there it remains a repository
secret, which the uploading job reads either way. So every job that invokes
the uploader declares that environment, no other job does, and no workflow a
pull request can start declares it in any job: a declaration there would let
branch code ask for the token once it has moved.

GitHub compares environment names without case, so the rules do too, and a
name computed by an expression is refused because its placement cannot be
proved.

The pull-request surface here is deliberately wider than
`codescene_coverage.pull_request_workflows`, which seeds from
`pull_request` and `pull_request_target` alone. An environment declaration
is only safe where no branch can start the job, so this closure also seeds
from review and comment events, the merge queue, `workflow_run` chains
(which run with secrets after whatever they name), and pushes not confined
to `main` or to tags, and it follows both `./` and `$/` local calls.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

from codescene_coverage import CODESCENE_ACTION, WORKFLOW_DIRECTORY
from workflow_reading import WorkflowReadingError, triggers, workflow_jobs

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    import collections.abc as cabc

    from workflow_reading import WorkflowDocument

#: The environment holding the CodeScene token, as GitHub folds it.
ENVIRONMENT: typ.Final[str] = "codescene"

#: Events that let a pull request, or a run it caused, start a workflow.
PULL_REQUEST_EVENTS: typ.Final[frozenset[str]] = frozenset({
    "pull_request",
    "pull_request_target",
    "pull_request_review",
    "pull_request_review_comment",
    "issue_comment",
    "merge_group",
    "workflow_run",
})

#: The only push filter keys that confine a push to tags.
TAG_FILTERS: typ.Final[frozenset[str]] = frozenset({"tags", "tags-ignore"})

MISSING: typ.Final[str] = f"the uploading job must declare `environment: {ENVIRONMENT}`"
STRAY: typ.Final[str] = f"declares `{ENVIRONMENT}` but uploads nothing"
REACHABLE: typ.Final[str] = (
    f"is reachable from a pull request and declares `{ENVIRONMENT}`"
)
UNRESOLVED: typ.Final[str] = (
    "names its environment with an expression, so its placement cannot be proved"
)
NO_UPLOADER: typ.Final[str] = "no workflow job invokes the CodeScene uploader"


def environment_name(job: dict[str, object]) -> str | None:
    """Return the environment a job declares, from either accepted form.

    Parameters
    ----------
    job : dict
        The job to read.

    Returns
    -------
    str or None
        The declared name, or None when the job declares none.

    Examples
    --------
    >>> environment_name({"environment": "codescene"})
    'codescene'
    >>> environment_name({"environment": {"name": "codescene", "url": "x"}})
    'codescene'
    >>> environment_name({}) is None
    True
    """
    match job.get("environment"):
        case str() as name:
            return name
        case {"name": str() as name}:
            return name
        case _:
            return None


def declares_codescene(job: dict[str, object]) -> bool:
    """Return whether a job declares the `codescene` environment, in any case.

    Parameters
    ----------
    job : dict
        The job to read.

    Returns
    -------
    bool
        True when the declared name folds to `codescene`.

    Examples
    --------
    >>> declares_codescene({"environment": "CodeScene"})
    True
    >>> declares_codescene({"environment": "production"})
    False
    """
    name = environment_name(job)
    return name is not None and name.casefold() == ENVIRONMENT


def has_unresolved_environment(job: dict[str, object]) -> bool:
    """Return whether a job computes its environment name with an expression.

    Parameters
    ----------
    job : dict
        The job to read.

    Returns
    -------
    bool
        True when the declared name contains `${{`.

    Examples
    --------
    >>> has_unresolved_environment({"environment": {"name": "${{ 'codescene' }}"}})
    True
    >>> has_unresolved_environment({"environment": "codescene"})
    False
    """
    name = environment_name(job)
    return name is not None and "${{" in name


def uploads(job: dict[str, object]) -> bool:
    """Return whether a job has a step invoking the shared uploader.

    The action path must match exactly, at any ref, so a look-alike action
    does not count.

    Parameters
    ----------
    job : dict
        The job to read.

    Returns
    -------
    bool
        True when some step's `uses` names the upload action.

    Examples
    --------
    >>> uploads({"steps": [{"uses": f"{CODESCENE_ACTION}@v1"}]})
    True
    >>> uploads({"steps": [{"uses": f"{CODESCENE_ACTION}-check@v1"}]})
    False
    """
    declared = job.get("steps")
    listed = declared if isinstance(declared, list) else []
    return any(
        isinstance(step, dict)
        and str(step.get("uses", "")).split("@", 1)[0] == CODESCENE_ACTION
        for step in listed
    )


def _pushes_other_branches(document: WorkflowDocument) -> bool:
    """Return whether a push trigger can run for a branch other than `main`.

    A push to a pull request's branch runs that branch's workflows, so any
    push filter not naming `main` alone, or tags alone, is pull-request
    surface. An unrecognised filter fails closed.
    """
    if "push" not in triggers(document):
        return False
    declared = document.get("on", document.get(True))
    filters = declared.get("push") if isinstance(declared, dict) else None
    if not isinstance(filters, dict) or not filters:
        return True
    if "branches" in filters:
        branches = filters["branches"]
        names = branches if isinstance(branches, list) else [branches]
        return {str(name) for name in names} != {"main"}
    return not set(filters) <= TAG_FILTERS


def is_pull_request_seed(document: WorkflowDocument) -> bool:
    """Return whether a pull request can start a workflow by its own triggers.

    Parameters
    ----------
    document : WorkflowDocument
        A parsed workflow.

    Returns
    -------
    bool
        True for a pull-request event or a push not confined to `main`.
    """
    return bool(triggers(document) & PULL_REQUEST_EVENTS) or _pushes_other_branches(
        document
    )


def _callee(reference: object, documents: dict[str, WorkflowDocument]) -> str | None:
    """Return the local workflow a job-level `uses:` names, if any."""
    if not isinstance(reference, str):
        return None
    path = reference.removeprefix("./").removeprefix("$/")
    if not path.startswith(WORKFLOW_DIRECTORY):
        return None
    name = path.removeprefix(WORKFLOW_DIRECTORY)
    return name if name in documents else None


def pull_request_closure(documents: dict[str, WorkflowDocument]) -> frozenset[str]:
    """Return every workflow a pull request can start, directly or through calls.

    Parameters
    ----------
    documents : dict
        File name to parsed document.

    Returns
    -------
    frozenset of str
        The seeds and every local workflow they call, transitively.

    Raises
    ------
    WorkflowReadingError
        If no workflow is a seed, which is the reader failing rather than
        the repository complying.
    """
    pending = [name for name, doc in documents.items() if is_pull_request_seed(doc)]
    if not pending:
        message = "no workflow serves a pull request; the trigger reader is broken"
        raise WorkflowReadingError(message, reader="pull_request_closure")
    reached: set[str] = set()
    while pending:
        name = pending.pop()
        if name in reached:
            continue
        reached.add(name)
        pending.extend(
            callee
            for job in workflow_jobs(documents[name]).values()
            if (callee := _callee(job.get("uses"), documents)) is not None
        )
    return frozenset(reached)


def _placed(
    documents: dict[str, WorkflowDocument], names: cabc.Iterable[str]
) -> list[tuple[str, dict[str, object]]]:
    """Return every job in the named workflows with its location."""
    return [
        (f"{name}:{job_id}", job)
        for name in sorted(names)
        for job_id, job in workflow_jobs(documents[name]).items()
    ]


def environment_violations(documents: dict[str, WorkflowDocument]) -> list[str]:
    """Report every departure from the `codescene` environment placement.

    Parameters
    ----------
    documents : dict
        File name to parsed document.

    Returns
    -------
    list of str
        One message per violation; empty when the placement holds.
    """
    placed = _placed(documents, documents)
    uploading = [(where, job) for where, job in placed if uploads(job)]
    if not uploading:
        return [NO_UPLOADER]
    problems = [
        f"{where}: {MISSING}" for where, job in uploading if not declares_codescene(job)
    ]
    problems.extend(
        f"{where} {STRAY}"
        for where, job in placed
        if not uploads(job) and declares_codescene(job)
    )
    problems.extend(
        f"{where} {UNRESOLVED}" for where, job in placed if has_unresolved_environment(job)
    )
    problems.extend(
        f"{where} {REACHABLE}"
        for where, job in _placed(documents, pull_request_closure(documents))
        if declares_codescene(job)
    )
    return problems
