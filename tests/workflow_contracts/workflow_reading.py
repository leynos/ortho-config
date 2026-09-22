"""Reading a GitHub Actions workflow, fallibly and visibly.

Generic: nothing here knows about CodeScene. The parsing, the trigger
grammar and the one filesystem call live together so that a contract
about any subject can use them, and so that neither this module nor the
CodeScene predicates beside it outgrows the 400-line limit
``AGENTS.md`` sets.

Only ``read_workflows`` touches the filesystem, and it takes the
directory to read rather than finding one, so every reading above it can
be driven with supplied documents. That is what lets a contract ask what
a rule makes of a workflow this repository does not contain: the real
files use one spelling of everything and cannot tell a working reader
from a broken one.
"""

from __future__ import annotations

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

    Examples
    --------
    >>> sorted(triggers(load_workflow("on:\\n  pull_request:\\n  push:\\n")))
    ['pull_request', 'push']
    >>> sorted(triggers(load_workflow("on: [push]")))
    ['push']
    >>> triggers(load_workflow("name: x\\n"))
    frozenset()
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

    Examples
    --------
    >>> serves_pull_requests(load_workflow("on:\\n  pull_request:\\n"))
    True
    >>> serves_pull_requests(load_workflow("on:\\n  push:\\n"))
    False
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

    Examples
    --------
    >>> pushes_to_main(load_workflow("on:\\n  push:\\n    branches: [main]\\n"))
    True
    >>> pushes_to_main(load_workflow("on:\\n  push:\\n    branches: [wip]\\n"))
    False
    >>> pushes_to_main(load_workflow("on:\\n  push:\\n    tags: ['v*']\\n"))
    False
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

    Examples
    --------
    >>> body = "jobs:\\n  a:\\n    steps:\\n      - run: echo one\\n"
    >>> [step["run"] for step in workflow_steps(load_workflow(body))]
    ['echo one']
    >>> workflow_steps(load_workflow("jobs: not-a-mapping\\n"))
    []
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
