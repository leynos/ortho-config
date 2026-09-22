"""What CV-005 asks of a repository's workflows, as predicates.

The CodeScene-specific half. The parsing and the trigger grammar are in
``workflow_reading``, which knows nothing about CodeScene; what is here
is the selection of subjects those rules apply to, and it is all pure
over supplied documents. What a workflow must not do once selected is
read in ``codescene_reach``.

Each reading treats finding nothing as a fault rather than an answer.
The rules built on them are refusals, and a refusal over an empty
subject set is satisfied by any repository at all.
"""

from __future__ import annotations

import re
import typing as typ

from workflow_reading import (
    WorkflowReadingError,
    pushes_to_main,
    serves_pull_requests,
    workflow_jobs,
    workflow_steps,
)

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from workflow_reading import WorkflowDocument


#: The action that talks to CodeScene, matched on its path rather than
#: on the word: the workflows discuss CodeScene in prose, and a comment
#: is not an invocation. A repin changes the SHA after the ``@``, so the
#: path is what stays true.
CODESCENE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage"
)

#: The coverage generator, which every lane may run.
COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage"
)

#: A full-length commit pin, which is the shape ``AGENTS.md`` asks a
#: contract to assert. The value is deliberately not named: Dependabot
#: owns these bumps, and a test holding today's SHA turns every routine
#: bump into a manual edit.
PINNED_COMMIT: typ.Final[re.Pattern[str]] = re.compile(r"^[0-9a-f]{40}$")

#: The command no pull-request lane may run.
CLI_COMMAND: typ.Final[str] = "cs-coverage"

#: The name of the environment variable no pull-request lane may put in
#: reach. The name rather than any value: the expression supplying it
#: may be a secret, a repository variable or a literal, and all three
#: reach the process the same way.
FORBIDDEN_VARIABLE: typ.Final[str] = "CS_ACCESS_TOKEN"


#: Where a same-repository reusable workflow lives, relative to the
#: repository root. GitHub resolves ``./.github/workflows/x.yml`` from
#: there, and a workflow outside this directory cannot be called at all.
WORKFLOW_DIRECTORY: typ.Final[str] = ".github/workflows/"


def _local_workflow(reference: object, documents: dict[str, WorkflowDocument]) -> str | None:
    """Return the file a ``uses:`` value names in this tree, if any.

    Matched by shape rather than by an enumerated prefix list: strip a
    leading ``./`` and ask whether what remains is a file directly under
    the workflow directory. A list of accepted spellings drops every
    spelling nobody thought to list, silently, while a pull request
    still runs the workflow it names; a cross-repository reference
    (``owner/repo/.github/workflows/x.yml@ref``) fails the shape because
    it does not start at the workflow directory.
    """
    if not isinstance(reference, str):
        return None
    path = reference.removeprefix("./")
    if not path.startswith(WORKFLOW_DIRECTORY):
        return None
    name = path.removeprefix(WORKFLOW_DIRECTORY)
    return name if name in documents else None


def called_workflows(
    document: WorkflowDocument, documents: dict[str, WorkflowDocument]
) -> frozenset[str]:
    """Return the same-repository reusable workflows one document calls.

    A reference to another repository is not followed. Its content is
    not in this tree, so nothing here could read it, and claiming to
    have checked it would be worse than saying plainly that it is out of
    scope.

    Parameters
    ----------
    document : WorkflowDocument
        The calling workflow.
    documents : dict
        Every workflow document, by file name.

    Returns
    -------
    frozenset of str
        The file names it calls, limited to documents present here.

    Examples
    --------
    >>> from workflow_reading import load_workflow
    >>> caller = load_workflow(
    ...     "jobs:\\n  call:\\n    uses: ./.github/workflows/probe.yml\\n"
    ... )
    >>> sorted(called_workflows(caller, {"probe.yml": {}}))
    ['probe.yml']
    >>> called_workflows(caller, {})
    frozenset()
    """
    names = (
        _local_workflow(job.get("uses"), documents)
        for job in workflow_jobs(document).values()
    )
    return frozenset(name for name in names if name is not None)


def _reachable(
    seeds: list[str], documents: dict[str, WorkflowDocument]
) -> dict[str, WorkflowDocument]:
    """Return the seeds and every workflow they call, transitively."""
    found: dict[str, WorkflowDocument] = {}
    pending = list(seeds)
    while pending:
        name = pending.pop()
        if name not in found:
            found[name] = documents[name]
            pending.extend(called_workflows(documents[name], documents) - found.keys())
    return found


def pull_request_workflows(
    documents: dict[str, WorkflowDocument],
) -> dict[str, WorkflowDocument]:
    """Return every workflow a pull request can reach.

    A closure rather than a trigger list. A workflow declaring only
    ``workflow_call`` still runs on a pull request when a pull-request
    workflow calls it, and ``secrets: inherit`` hands it the token, so a
    reading that enumerated triggers alone could not see it: every
    refusal below would pass over it while it did the forbidden thing.

    Measured on episodic rather than argued: a ``workflow_call``
    workflow curling the CodeScene project API with an inherited
    ``CS_ACCESS_TOKEN``, called from a pull-request job, passed every
    clause of the equivalent contract there.

    Parameters
    ----------
    documents : dict
        File name to parsed document.

    Returns
    -------
    dict
        Every workflow reachable from a pull request: those declaring a
        pull-request trigger, and everything they call, transitively.

    Raises
    ------
    WorkflowReadingError
        If none does, which cannot be true of a repository with a
        pull-request lane.

    Examples
    --------
    >>> from workflow_reading import load_workflow
    >>> documents = {
    ...     "ci.yml": load_workflow("on:\\n  pull_request:\\n"),
    ...     "main.yml": load_workflow("on:\\n  push:\\n"),
    ... }
    >>> sorted(pull_request_workflows(documents))
    ['ci.yml']
    """
    found = _reachable(
        [name for name, document in documents.items() if serves_pull_requests(document)],
        documents,
    )
    if not found:
        message = (
            "this reading found no workflow serving a pull request; the "
            "trigger reader is broken, not the workflows"
        )
        raise WorkflowReadingError(message, reader="pull_request_workflows")
    return found


def publishers(
    documents: dict[str, WorkflowDocument],
) -> dict[str, WorkflowDocument]:
    """Return the workflows allowed to upload coverage.

    A publisher pushes to main *and serves no pull request*. Both halves
    are needed and the second is easy to drop: a repository's main
    workflow often declares ``pull_request`` and ``push`` together, so a
    predicate reading only the push makes one file simultaneously
    required to upload and forbidden from uploading.

    Parameters
    ----------
    documents : dict
        File name to parsed document.

    Returns
    -------
    dict
        The subset allowed to publish. Empty is a legitimate answer
        rather than a fault, because "no publisher" is one of the states
        the contract exists to refuse.

    Examples
    --------
    >>> from workflow_reading import load_workflow
    >>> main = load_workflow("on:\\n  push:\\n    branches: [main]\\n")
    >>> sorted(publishers({"main.yml": main}))
    ['main.yml']
    >>> both = load_workflow("on:\\n  pull_request:\\n  push:\\n")
    >>> publishers({"ci.yml": both})
    {}
    """
    return {
        name: document
        for name, document in documents.items()
        if pushes_to_main(document) and not serves_pull_requests(document)
    }


def coverage_steps(
    documents: dict[str, WorkflowDocument],
) -> dict[str, list[dict[str, object]]]:
    """Return each workflow's generate-coverage steps, keyed by file name.

    A list rather than one step, because this repository runs two
    feature legs per lane and both are subjects of the rules below.

    Parameters
    ----------
    documents : dict
        File name to parsed document.

    Returns
    -------
    dict
        File name to its coverage steps, in declaration order.

    Examples
    --------
    >>> from workflow_reading import load_workflow
    >>> body = (
    ...     "jobs:\\n  a:\\n    steps:\\n"
    ...     f"      - uses: {COVERAGE_ACTION}@" + "a" * 40 + "\\n"
    ... )
    >>> {name: len(steps) for name, steps in
    ...  coverage_steps({"ci.yml": load_workflow(body)}).items()}
    {'ci.yml': 1}
    """
    found: dict[str, list[dict[str, object]]] = {}
    for name, document in documents.items():
        steps = [
            step
            for step in workflow_steps(document)
            if COVERAGE_ACTION in str(step.get("uses", ""))
        ]
        if steps:
            found[name] = steps
    return found
