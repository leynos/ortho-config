"""What puts CodeScene in reach of a lane, as readings.

Separated from ``codescene_coverage``, whose subject is which workflows
a rule applies to, so neither module outgrows the 400-line limit
``AGENTS.md`` sets. The subject here is what a workflow does that a
pull-request lane must not: put the secret in reach of a process, or
name the service's host. Every reading is pure over a supplied
document and structural rather than textual, because the workflows
explain in prose why the CodeScene check is gone and a comment is not
access.
"""

from __future__ import annotations

import re
import typing as typ

from codescene_coverage import FORBIDDEN_VARIABLE
from workflow_reading import workflow_jobs

if typ.TYPE_CHECKING:  # pragma: no cover - typing only
    from collections.abc import Iterator

    from workflow_reading import WorkflowDocument


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


def _step_token_sites(where: str, step: dict[str, object]) -> list[str]:
    """Return the places one step reads the secret: env, inputs, script."""
    inputs = step.get("with")
    found = {
        "env": _environment_names(step.get("env")),
        "inputs": isinstance(inputs, dict) and any(_reads(v) for v in inputs.values()),
        "run": _reads(step.get("run", "")),
    }
    return [f"{where} {scope}" for scope, reads in found.items() if reads]


def _job_token_sites(name: str, job_name: str, job: dict[str, object]) -> list[str]:
    """Return every place one job puts the forbidden variable in reach."""
    sites = [f"{name}: job {job_name} env"] if _environment_names(job.get("env")) else []
    sites += _forwarding_sites(name, job_name, job.get("secrets"))
    steps = job.get("steps")
    for index, step in enumerate(steps if isinstance(steps, list) else []):
        if isinstance(step, dict):
            sites += _step_token_sites(f"{name}: job {job_name} step {index + 1}", step)
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
    for job_name, job in workflow_jobs(document).items():
        sites += _job_token_sites(name, job_name, job)
    return sites


#: The service itself. A pull-request lane that reaches it by any other
#: road than the action (``curl`` in a script, a third-party action's
#: input, a URL in an environment variable or a reusable workflow's
#: input) escapes the action and command clauses alike, and escapes the
#: token clause too when the credential travels under another name.
CODESCENE_HOST: typ.Final[str] = "codescene.io"


def _scalars(value: object, where: str) -> Iterator[tuple[str, str]]:
    """Yield every scalar in a parsed document with the path that reaches it."""
    match value:
        case dict():
            for key, child in value.items():
                yield from _scalars(child, f"{where}.{key}" if where else str(key))
        case list():
            for index, child in enumerate(value):
                yield from _scalars(child, f"{where}[{index}]")
        case _:
            yield where, str(value)


def codescene_contacts(name: str, document: WorkflowDocument) -> list[str]:
    """Return every place in one workflow that names the CodeScene host.

    Every value in the parsed document is read, at every scope, rather
    than a list of the places a contact is expected. A URL can reach a
    step through the workflow's ``env``, a job's ``env``, a step's
    script, inputs or ``env``, or a reusable-workflow call's ``with``,
    and a reading that enumerated some of those scopes left the rest as
    a way round the rule. A comment explaining why a lane no longer
    talks to CodeScene is not read as the lane talking to it, because
    the parser discards comments.

    Parameters
    ----------
    name : str
        The workflow's file name, for the message.
    document : WorkflowDocument
        The parsed workflow.

    Returns
    -------
    list of str
        One entry per value naming the host, with its path.

    Examples
    --------
    >>> from workflow_reading import load_workflow
    >>> body = "jobs:\\n  a:\\n    steps:\\n      - run: curl https://api.codescene.io\\n"
    >>> codescene_contacts("ci.yml", load_workflow(body))
    ['ci.yml: jobs.a.steps[0].run']
    >>> codescene_contacts("ci.yml", load_workflow("# see codescene.io\\nname: x\\n"))
    []
    """
    return [
        f"{name}: {where}"
        for where, text in _scalars(document, "")
        if CODESCENE_HOST in text
    ]
