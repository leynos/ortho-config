"""What CV-005 asks of a repository's workflows, as predicates.

The CodeScene-specific half. The parsing and the trigger grammar are in
``workflow_reading``, which knows nothing about CodeScene; what is here
is the selection of subjects those rules apply to, and it is all pure
over supplied documents.

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


def pull_request_workflows(
    documents: dict[str, WorkflowDocument],
) -> dict[str, WorkflowDocument]:
    """Return the workflows that serve pull requests.

    Parameters
    ----------
    documents : dict
        File name to parsed document.

    Returns
    -------
    dict
        The subset serving pull requests.

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
    found = {
        name: document
        for name, document in documents.items()
        if serves_pull_requests(document)
    }
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


def _environment_names(mapping: object) -> bool:
    """Return whether an ``env`` mapping declares the forbidden variable."""
    return isinstance(mapping, dict) and FORBIDDEN_VARIABLE in mapping


def _job_token_sites(name: str, job_name: str, job: dict[str, object]) -> list[str]:
    """Return every place one job puts the forbidden variable in reach."""
    sites = [f"{name}: job {job_name} env"] if _environment_names(job.get("env")) else []
    steps = job.get("steps")
    if not isinstance(steps, list):
        return sites
    return sites + [
        f"{name}: job {job_name} step {index + 1} env"
        for index, step in enumerate(steps)
        if isinstance(step, dict) and _environment_names(step.get("env"))
    ]


def token_sites(name: str, document: WorkflowDocument) -> list[str]:
    """Return every place one workflow puts the forbidden variable in reach.

    Structural rather than textual, and the distinction is the point.
    Both workflows explain in prose why the CodeScene check is gone, and
    that explanation names the variable; a sweep over the raw file would
    read the explanation as the violation and push the next person into
    deleting the reason rather than the reference.

    Parameters
    ----------
    name : str
        The workflow's file name, for the message.
    document : WorkflowDocument
        The parsed workflow.

    Returns
    -------
    list of str
        One entry per site, naming where it is.

    Examples
    --------
    >>> from workflow_reading import load_workflow
    >>> body = f"env:\\n  {FORBIDDEN_VARIABLE}: x\\njobs:\\n  a:\\n    steps: []\\n"
    >>> token_sites("ci.yml", load_workflow(body))
    ['ci.yml: workflow env']
    >>> token_sites("ci.yml", load_workflow("jobs:\\n  a:\\n    steps: []\\n"))
    []
    """
    sites = [f"{name}: workflow env"] if _environment_names(document.get("env")) else []
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        return sites
    for job_name, job in jobs.items():
        if isinstance(job, dict):
            sites += _job_token_sites(name, str(job_name), job)
    return sites
