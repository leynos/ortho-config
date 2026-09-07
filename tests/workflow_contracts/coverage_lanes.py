"""Reads every coverage-invoking job out of the workflow files.

Separated from ``timeout_budgets`` so the workflow reading and the
nextest arithmetic stay legible apart, and so neither module outgrows
the 400-line limit ``AGENTS.md`` sets.
"""

from __future__ import annotations

import typing as typ

import yaml
from timeout_budgets import COVERAGE_ACTION, WATCHDOG_VARIABLE, WORKFLOWS_DIRECTORY

class CoverageJob(typ.NamedTuple):
    """One job that invokes the coverage action, with its budgets.

    Attributes
    ----------
    workflow : str
        The workflow file's name.
    job : str
        The job's identifier.
    steps : int
        How many coverage steps the job runs. Each gets its own watchdog,
        so the job must contain all of their budgets.
    watchdogs : tuple[float | None, ...]
        The watchdog budget in force for each of those steps, in order,
        with None where neither the step nor the job sets one.
    job_timeout : float or None
        The job's ``timeout-minutes`` in seconds, or None when it
        declares none and so inherits GitHub's six-hour default.
    """

    workflow: str
    job: str
    steps: int
    watchdogs: tuple[float | None, ...]
    job_timeout: float | None
    conditions: tuple[tuple[object, object], ...] = ()

    def __str__(self) -> str:
        """Return a location suitable for a failure message.

        Returns
        -------
        str
            ``workflow:job`` for this job.
        """
        return f"{self.workflow}:{self.job}"


def _watchdog_of(job: dict[str, typ.Any], step: dict[str, typ.Any]) -> float | None:
    """Return the watchdog budget in force for one step.

    A step's own environment wins over the job's, as GitHub resolves it,
    so a step that overrode the job value is read as it will run rather
    than as the job declares.

    Parameters
    ----------
    job : dict[str, typ.Any]
        The enclosing job.
    step : dict[str, typ.Any]
        The coverage step.

    Returns
    -------
    float or None
        The budget in seconds, or None when neither sets one.
    """
    for owner in (step, job):
        environment = owner.get("env")
        if not isinstance(environment, dict):
            continue
        budget = _budget_from(environment.get(WATCHDOG_VARIABLE))
        if budget is not None:
            return budget
    return None


def _budget_from(raw: object) -> float | None:
    """Return one source's watchdog budget, or None when it sets none."""
    if raw is None:
        return None
    text = str(raw).strip()
    if not text:
        return None
    try:
        seconds = float(text)
    except ValueError:
        return None
    return seconds if seconds > 0 else None


def workflow_documents() -> dict[str, dict[str, typ.Any]]:
    """Return every workflow document in the repository, keyed by name.

    This is the one place the contract touches the filesystem or the
    YAML parser, so an unreadable or unparsable workflow fails here
    rather than inside a budget derivation several frames away.
    Both extensions are read. A coverage lane in the other one would
    otherwise escape every assertion below without failing anything.

    Returns
    -------
    dict[str, dict[str, typ.Any]]
        File name to parsed document.
    """
    documents: dict[str, dict[str, typ.Any]] = {}
    for pattern in ("*.yml", "*.yaml"):
        for path in sorted(WORKFLOWS_DIRECTORY.glob(pattern)):
            parsed = yaml.safe_load(path.read_text(encoding="utf-8"))
            if isinstance(parsed, dict):
                documents[path.name] = parsed
    return documents


def _jobs_in(document: dict[str, typ.Any]) -> dict[str, dict[str, typ.Any]]:
    """Return a document's jobs, ignoring anything that is not one."""
    jobs = document.get("jobs")
    if not isinstance(jobs, dict):
        return {}
    return {
        str(name): job for name, job in jobs.items() if isinstance(job, dict)
    }


#: What GitHub accepts as a number of minutes. `bool` is excluded
#: rather than merely unlisted, because it is an `int` in Python and
#: `timeout-minutes: true` would otherwise read as one minute.
_MINUTE_TYPES: typ.Final[tuple[type, ...]] = (int, float, str)


def _is_minutes(raw: object) -> bool:
    """Return whether a value could be a number of minutes."""
    return isinstance(raw, _MINUTE_TYPES) and not isinstance(raw, bool)


def _ceiling_seconds(raw: object) -> float | None:
    """Return a job's ``timeout-minutes`` in seconds, or None."""
    if not _is_minutes(raw):
        return None
    try:
        return float(typ.cast("int | float | str", raw)) * 60.0
    except ValueError:
        return None


def _coverage_steps(job: dict[str, typ.Any]) -> list[dict[str, typ.Any]]:
    """Return one job's coverage steps, in the order it runs them."""
    steps = job.get("steps")
    if not isinstance(steps, list):
        return []
    return [
        step
        for step in steps
        if isinstance(step, dict) and COVERAGE_ACTION in str(step.get("uses", ""))
    ]


def _coverage_job(
    workflow: str, job_name: str, job: dict[str, typ.Any]
) -> CoverageJob | None:
    """Return one job's budgets, or None when it runs no coverage step."""
    steps = _coverage_steps(job)
    if not steps:
        return None
    raw_timeout = job.get("timeout-minutes")
    return CoverageJob(
        workflow=workflow,
        job=job_name,
        steps=len(steps),
        watchdogs=tuple(_watchdog_of(job, step) for step in steps),
        job_timeout=_ceiling_seconds(raw_timeout),
        conditions=tuple((step.get("if"), job.get("if")) for step in steps),
    )


def coverage_jobs_of(
    documents: dict[str, dict[str, typ.Any]] | None = None,
) -> tuple[CoverageJob, ...]:
    """Return every job invoking the coverage action, with its budgets.

    Jobs are the unit rather than steps, because the ceiling is a job's
    and it has to contain every watchdog inside it. Counting steps is
    what makes the two invocations here visible to the arithmetic.

    The documents are a parameter so the reading can be driven with
    synthetic workflows. Reading the repository's own is the default
    rather than the only option, which keeps the filesystem access and
    the YAML parsing at one named boundary instead of inside the
    derivations.

    Parameters
    ----------
    documents : dict[str, dict[str, typ.Any]] or None
        Parsed workflow documents keyed by file name. When None, the
        repository's own `.github/workflows` is read.

    Returns
    -------
    tuple[CoverageJob, ...]
        One entry per coverage-invoking job.
    """
    if documents is None:
        documents = workflow_documents()
    return tuple(
        found
        for name, document in documents.items()
        for job_name, job in _jobs_in(document).items()
        if (found := _coverage_job(name, str(job_name), job)) is not None
    )


