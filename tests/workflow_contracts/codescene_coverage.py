"""Reading the CodeScene coverage shape CV-005 requires.

Separated from ``codescene_coverage_test`` so the reading and the
assertions over it stay legible apart, and so neither module outgrows
the 400-line limit ``AGENTS.md`` sets.

The acquisition is one function and everything above it is pure. Only
``read_workflows`` touches the filesystem, and it takes the directory to
read rather than finding one, so every policy reading below can be
driven with supplied documents. That is what lets a contract ask what
these rules make of a workflow this repository does not contain: the
real files use one spelling of everything and cannot tell a working
reader from a broken one.

Each policy reading treats finding nothing as a fault rather than an
answer. The rules built on them are refusals, and a refusal over an
empty subject set is satisfied by any repository at all.
"""

from __future__ import annotations

import re
import typing as typ

import yaml

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    import pathlib

#: A parsed workflow document.
#:
#: The key type is ``str | bool`` rather than ``str``, and that is a
#: statement about YAML rather than defensiveness. YAML 1.1 resolves an
#: unquoted ``on:`` to a boolean, so a loader that resolves scalars keys
#: every workflow in this estate under ``True``. ``load_workflow`` uses
#: ``yaml.BaseLoader`` and keeps the string, but a reader that declared
#: ``dict[str, object]`` and then looked under ``True`` would be
#: claiming something the type says cannot happen.
WorkflowDocument = dict[typ.Union[str, bool], object]

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

#: Triggers that mean a workflow serves pull requests. ``pull_request``
#: and ``pull_request_target`` both run with a pull request's head in
#: view, and the second runs with the base repository's secrets, which
#: is the more dangerous of the two to give a token to.
PULL_REQUEST_TRIGGERS: typ.Final[frozenset[str]] = frozenset(
    {"pull_request", "pull_request_target"}
)


class WorkflowReadingError(RuntimeError):
    """Raised when a reading here finds nothing it must have found.

    Every rule built on these readings is a refusal, and a refusal over
    an empty subject set is satisfied by any repository at all. So an
    empty reading is reported as a fault of the reader rather than
    returned, and it is a distinct type so that "this reader is broken"
    and "this repository complies" cannot be confused.

    Attributes
    ----------
    reader : str
        The reading that failed, named as the caller would name it.
    path : str or None
        What the reading was over, when it was over something nameable.
    """

    def __init__(self, message: str, *, reader: str, path: str | None = None) -> None:
        """Record the message, the reading, and what it was over.

        Parameters
        ----------
        message : str
            What went wrong, for a person reading the failure.
        reader : str
            The reading that failed.
        path : str or None
            The directory or file name the fault is about, if any.
        """
        super().__init__(message)
        self.reader = reader
        self.path = path


def load_workflow(text: str) -> WorkflowDocument:
    r"""Parse a workflow while retaining every scalar as a string.

    Parameters
    ----------
    text : str
        The workflow document's YAML.

    Returns
    -------
    WorkflowDocument
        Its top-level mapping.

    Raises
    ------
    TypeError
        If the document does not parse to a mapping.

    Examples
    --------
    >>> load_workflow("on:\n  push:\n")
    {'on': {'push': ''}}
    """
    parsed = yaml.load(text, Loader=yaml.BaseLoader)  # noqa: S506 - BaseLoader is safe
    if not isinstance(parsed, dict):
        message = "a workflow must parse to a top-level mapping"
        raise TypeError(message)
    return typ.cast("WorkflowDocument", parsed)


def triggers(document: WorkflowDocument) -> frozenset[str]:
    """Return the trigger names a workflow declares.

    ``on`` is read through both the string key and the boolean ``True``,
    and this is not defensive noise. YAML 1.1 resolves an unquoted
    ``on:`` to a boolean, so a loader that resolves scalars gives every
    workflow a ``True`` key and none named ``on``. A reader looking only
    for the string then reports every workflow as having no triggers,
    which makes every rule built on it iterate an empty set and pass.

    Parameters
    ----------
    document : WorkflowDocument
        A parsed workflow.

    Returns
    -------
    frozenset of str
        Its trigger names, in whichever of the mapping, list and
        bare-string forms the document uses.
    """
    declared = document.get("on", document.get(True))
    if isinstance(declared, dict | list):
        return frozenset(str(name) for name in declared)
    if isinstance(declared, str):
        return frozenset({declared})
    return frozenset()


def _names(value: object) -> list[str]:
    """Return a filter's entries, which GitHub spells as a list or a scalar."""
    if isinstance(value, list):
        return [str(entry) for entry in value]
    return [value] if isinstance(value, str) else []


def serves_pull_requests(document: WorkflowDocument) -> bool:
    """Return whether a workflow runs for a pull request.

    Parameters
    ----------
    document : WorkflowDocument
        A parsed workflow.

    Returns
    -------
    bool
        True when it declares ``pull_request`` or ``pull_request_target``.
    """
    return bool(triggers(document) & PULL_REQUEST_TRIGGERS)


def pushes_to_main(document: WorkflowDocument) -> bool:
    """Return whether a workflow runs on a push to the main branch.

    Every filter form GitHub accepts is answered, and an unrecognised
    one is answered ``False`` rather than ``True``. Failing closed is
    the safe direction: this reading decides which workflow may publish
    coverage, so a shape it cannot understand must not be granted that
    permission by default.

    Parameters
    ----------
    document : WorkflowDocument
        A parsed workflow.

    Returns
    -------
    bool
        True when a push to ``main`` runs this workflow.
    """
    if "push" not in triggers(document):
        return False
    declared = document.get("on", document.get(True))
    filters = declared.get("push") if isinstance(declared, dict) else None
    if not isinstance(filters, dict) or not filters:
        return True
    if "branches" in filters:
        return "main" in _names(filters["branches"])
    if "branches-ignore" in filters:
        return "main" not in _names(filters["branches-ignore"])
    # A tag filter alone leaves the trigger firing for tag pushes only,
    # and a tag push is not a branch push. Every other filter, `paths`
    # among them, narrows which pushes run the workflow without
    # excluding main, so the answer there is still yes.
    return not {"tags", "tags-ignore"} & set(filters)


def workflow_steps(document: WorkflowDocument) -> list[dict[str, object]]:
    """Return every step of every job in one workflow.

    A job or step that is not a mapping is dropped rather than raising.
    A malformed fragment is not this reading's subject, and refusing the
    whole document over one would let an unrelated workflow fail a
    contract about coverage.

    Parameters
    ----------
    document : WorkflowDocument
        A parsed workflow.

    Returns
    -------
    list of dict
        Every step, flattened across jobs in declaration order.
    """
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        return []
    return [
        step
        for job in jobs.values()
        if isinstance(job, dict)
        for step in (job.get("steps") or [])
        if isinstance(step, dict)
    ]


def _read_one(path: pathlib.Path) -> WorkflowDocument:
    """Return one workflow's parsed document, naming the file on failure."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        message = f"{path} could not be read: {error}"
        raise WorkflowReadingError(
            message, reader="read_workflows", path=str(path)
        ) from error
    try:
        return load_workflow(text)
    except (TypeError, ValueError) as error:
        message = f"{path} is not a workflow document: {error}"
        raise WorkflowReadingError(
            message, reader="read_workflows", path=str(path)
        ) from error


def read_workflows(directory: pathlib.Path) -> dict[str, WorkflowDocument]:
    """Return every workflow document under one directory.

    The only filesystem access here. It takes the directory rather than
    finding one, so a caller can point it at a fixture tree and every
    reading below can be driven without it.

    Parameters
    ----------
    directory : pathlib.Path
        The directory to read.

    Returns
    -------
    dict
        File name to parsed document. Both suffixes are read: GitHub
        runs a workflow named either way, so a sweep over one of them
        reports repository-wide coverage while ignoring half the places
        a lane can be declared.

    Raises
    ------
    WorkflowReadingError
        If the directory holds no workflow, or one cannot be read or
        parsed.
    """
    found = {
        path.name: _read_one(path)
        for pattern in ("*.yml", "*.yaml")
        for path in sorted(directory.glob(pattern))
    }
    if not found:
        message = (
            f"no workflow documents were read from {directory}; every "
            f"assertion built on this reading is satisfied by finding "
            f"nothing, so this is the reader failing rather than the "
            f"repository complying"
        )
        raise WorkflowReadingError(
            message, reader="read_workflows", path=str(directory)
        )
    return found


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
    """
    sites = [f"{name}: workflow env"] if _environment_names(document.get("env")) else []
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        return sites
    for job_name, job in jobs.items():
        if isinstance(job, dict):
            sites += _job_token_sites(name, str(job_name), job)
    return sites
