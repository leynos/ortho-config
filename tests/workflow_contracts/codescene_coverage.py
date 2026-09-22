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


def called_workflows(
    document: WorkflowDocument, documents: dict[str, WorkflowDocument]
) -> frozenset[str]:
    """Return the same-repository reusable workflows one document calls.

    GitHub accepts two spellings for a local reusable workflow,
    ``./.github/workflows/x.yml`` and ``$/.github/workflows/x.yml``, the
    second being the documented recommendation. A reader that knows only
    the first silently drops callers written the other way.

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
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        return frozenset()
    called: set[str] = set()
    for job in jobs.values():
        if not isinstance(job, dict):
            continue
        reference = job.get("uses")
        if not isinstance(reference, str):
            continue
        for prefix in ("./", "$/"):
            if reference.startswith(prefix):
                name = reference.removeprefix(prefix).rsplit("/", 1)[-1]
                if name in documents:
                    called.add(name)
    return frozenset(called)


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
    found: dict[str, WorkflowDocument] = {}
    pending = [
        name for name, document in documents.items() if serves_pull_requests(document)
    ]
    while pending:
        name = pending.pop()
        if name in found:
            continue
        found[name] = documents[name]
        pending.extend(called_workflows(documents[name], documents) - found.keys())
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


#: An expression reading the secret, under any context GitHub resolves
#: one from. Matched on the reference rather than the bare name so that
#: prose naming the variable is not mistaken for access: what puts it in
#: reach is ``${{ secrets.NAME }}``.
_REFERENCE: typ.Final[re.Pattern[str]] = re.compile(
    rf"\$\{{\{{\s*(?:secrets|env|vars)\.{re.escape(FORBIDDEN_VARIABLE)}\s*\}}\}}"
)


def _reads(value: object) -> bool:
    """Return whether a value reads the secret by reference."""
    return bool(_REFERENCE.search(str(value)))


def _environment_names(mapping: object) -> bool:
    """Return whether an ``env`` mapping declares or forwards the variable."""
    if not isinstance(mapping, dict):
        return False
    return FORBIDDEN_VARIABLE in mapping or any(
        _reads(value) for value in mapping.values()
    )


def _forwarding_sites(name: str, job_name: str, forwarded: object) -> list[str]:
    """Return sites where a reusable-workflow call forwards the secret.

    ``secrets: inherit`` is the sharp one. It names nothing, so a
    reading looking for the variable finds no mention of it while the
    called workflow receives every secret the caller holds.
    """
    match forwarded:
        case str() if forwarded.strip() == "inherit":
            return [f"{name}: job {job_name} secrets: inherit"]
        case dict():
            return [
                f"{name}: job {job_name} secrets {key}"
                for key, value in forwarded.items()
                if key == FORBIDDEN_VARIABLE or _reads(value)
            ]
        case _:
            return []


def _job_token_sites(name: str, job_name: str, job: dict[str, object]) -> list[str]:
    """Return every place one job puts the forbidden variable in reach."""
    sites = [f"{name}: job {job_name} env"] if _environment_names(job.get("env")) else []
    sites += _forwarding_sites(name, job_name, job.get("secrets"))
    steps = job.get("steps")
    if not isinstance(steps, list):
        return sites
    for index, step in enumerate(steps):
        if not isinstance(step, dict):
            continue
        where = f"job {job_name} step {index + 1}"
        if _environment_names(step.get("env")):
            sites.append(f"{name}: {where} env")
        inputs = step.get("with")
        if isinstance(inputs, dict) and any(_reads(v) for v in inputs.values()):
            sites.append(f"{name}: {where} inputs")
        if _reads(step.get("run", "")):
            sites.append(f"{name}: {where} run")
    return sites


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
