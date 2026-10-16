"""Reading which Linux jobs run the suite, for the build-tools provisioning contract.

The pinned build tools have to be installed before any job can reach the
preflight or run the suite on Linux. Which jobs those are is derived from the
workflows and the Makefile, not listed by hand: a list of today's job names
passes when a new job arrives, which is the case it exists for.

Three readings do the work:

- ``legs`` expands a job's matrix into the concrete runs it makes, because one
  job can be a Linux leg and a Windows leg at once.
- ``runner_labels`` resolves ``runs-on`` in each of the forms GitHub accepts
  (a scalar, a list, a ``group``/``labels`` mapping) and through a matrix
  reference, including the fork fallback that names two runners in one
  expression. An expression it cannot resolve raises: a reader that treats an
  unknown runner as "not Linux" would pass a job it never understood.
- ``reaches_the_suite`` says whether a step runs tests, either directly or by
  asking Make, whose dry run is the only honest answer to "does this goal reach
  the preflight".

Every reading over-approximates reachability. A step whose condition it cannot
evaluate is treated as running, because the safe error is demanding an
installer that was not needed, not excusing one that was.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import dataclasses
import itertools
import re
import shlex
import typing as typ

from build_standard_make import PREFLIGHT_SCRIPT, dry_run

#: Make variables a workflow may set per step or per job to change a route.
MAKE_SWITCHES: typ.Final[tuple[str, ...]] = ("BUILD_STANDARD",)

#: Direct invocations that run tests (``cargo install`` and friends do not).
_DIRECT_SUITE: typ.Final[re.Pattern[str]] = re.compile(
    r"\bcargo(?:\s+\+\S+)?\s+(?:llvm-cov|nextest|test|mutants)\b"
)
_GENERATE_COVERAGE: typ.Final[str] = "/generate-coverage@"
_MATRIX_REFERENCE: typ.Final[re.Pattern[str]] = re.compile(r"matrix\.([A-Za-z0-9_-]+)")
_QUOTED_LITERAL: typ.Final[re.Pattern[str]] = re.compile(r"'([^']+)'")
#: References that can choose a runner but cannot be known from the workflow text.
_UNRESOLVABLE: typ.Final[re.Pattern[str]] = re.compile(r"\b(?:inputs|vars|env|needs|steps|secrets)\.")
_MATRIX_COMPARISON: typ.Final[re.Pattern[str]] = re.compile(
    r"matrix\.([A-Za-z0-9_-]+)\s*(==|!=)\s*'([^']*)'"
)


class UnreadableRunnerError(RuntimeError):
    """Raised when ``runs-on`` cannot be resolved, so no verdict is possible."""


@dataclasses.dataclass(frozen=True)
class Leg:
    """One concrete run of a job: a job id and one cell of its matrix.

    Attributes
    ----------
    workflow : str
        The workflow file name.
    job_id : str
        The job's key.
    cell : dict
        The matrix values this leg runs with (empty for a job with no matrix).
    job : dict
        The parsed job mapping.
    """

    workflow: str
    job_id: str
    cell: dict[str, str]
    job: dict[str, object]

    @property
    def name(self) -> str:
        """Return a stable label such as ``ci.yml:build-test:linux``."""
        suffix = self.cell.get("platform") or self.cell.get("os") or ""
        return f"{self.workflow}:{self.job_id}:{suffix}".rstrip(":")


def legs(workflow: str, job_id: str, job: dict[str, object]) -> list[Leg]:
    """Expand *job*'s matrix into its legs.

    Parameters
    ----------
    workflow : str
        The workflow file name, for labelling.
    job_id : str
        The job's key.
    job : dict
        The parsed job.

    Returns
    -------
    list of Leg
        One per matrix cell; a single leg when the job has no matrix.
    """
    strategy = job.get("strategy")
    matrix = strategy.get("matrix") if isinstance(strategy, dict) else None
    if not isinstance(matrix, dict):
        return [Leg(workflow, job_id, {}, job)]
    axes = {
        key: value
        for key, value in matrix.items()
        if key not in {"include", "exclude"} and isinstance(value, list)
    }
    cells: list[dict[str, str]] = [
        dict(zip(axes, combination, strict=True))
        for combination in itertools.product(*axes.values())
    ] if axes else []
    include = matrix.get("include")
    cells += [
        {str(key): str(value) for key, value in entry.items()}
        for entry in (include if isinstance(include, list) else [])
        if isinstance(entry, dict)
    ]
    return [Leg(workflow, job_id, cell, job) for cell in cells] or [Leg(workflow, job_id, {}, job)]


def runner_labels(leg: Leg) -> list[str]:
    """Return every runner label the leg can land on.

    Parameters
    ----------
    leg : Leg
        The leg to resolve.

    Returns
    -------
    list of str
        Labels from a scalar, list or ``labels`` mapping, with each
        ``matrix.<key>`` reference replaced by the leg's value. An expression
        naming two runners (the fork fallback) yields both.

    Raises
    ------
    UnreadableRunnerError
        If ``runs-on`` is absent or its expression names anything but the
        matrix.
    """
    runs_on = leg.job.get("runs-on")
    raw: list[str]
    if isinstance(runs_on, str):
        raw = [runs_on]
    elif isinstance(runs_on, list):
        raw = [str(label) for label in runs_on]
    elif isinstance(runs_on, dict):
        labels = runs_on.get("labels", [])
        raw = [labels] if isinstance(labels, str) else [str(label) for label in labels]
        raw += [str(runs_on["group"])] if "group" in runs_on else []
    else:
        message = f"{leg.name} declares no readable runs-on: {runs_on!r}"
        raise UnreadableRunnerError(message)
    resolved: list[str] = []
    for label in raw:
        if "${{" not in label:
            resolved.append(label)
            continue
        keys = _MATRIX_REFERENCE.findall(label)
        literals = _QUOTED_LITERAL.findall(label)
        remainder = _MATRIX_REFERENCE.sub("", label)
        # `github.*` can only be a condition (the fork fallback tests the event);
        # these cannot pick a runner label, so they are not a reason to refuse.
        if (not keys and not literals) or _UNRESOLVABLE.search(remainder):
            message = f"{leg.name} runs-on cannot be resolved to labels: {label!r}"
            raise UnreadableRunnerError(message)
        missing = [key for key in keys if key not in leg.cell]
        if missing:
            message = f"{leg.name} runs-on names matrix keys the leg lacks: {missing}"
            raise UnreadableRunnerError(message)
        resolved += [leg.cell[key] for key in keys] + literals
    return resolved


def is_linux(label: str) -> bool:
    """Report whether a runner label names a Linux runner.

    Examples
    --------
    >>> [is_linux(label) for label in ("ubuntu-latest", "ubicloud-standard-4", "windows-latest", "macos-15")]
    [True, True, False, False]
    """
    lowered = label.lower()
    return lowered.startswith(("ubuntu", "ubicloud")) or "linux" in lowered


def runs_on_linux(leg: Leg) -> bool:
    """Report whether the leg can land on a Linux runner."""
    return any(is_linux(label) for label in runner_labels(leg))


def step_runs_on(step: dict[str, object], cell: dict[str, str]) -> bool:
    """Report whether a step's ``if`` lets it run on the leg with matrix *cell*.

    Only comparisons against ``matrix.<key>`` are evaluated, joined by ``&&``.
    Anything else (``always()``, ``runner.environment``, an ``||``) is treated as
    true, the safe reading for a contract that demands installers.

    Examples
    --------
    >>> step_runs_on({"if": "${{ matrix.platform == 'linux' }}"}, {"platform": "windows"})
    False
    >>> step_runs_on({"if": "${{ always() && matrix.platform != 'linux' }}"}, {"platform": "windows"})
    True
    >>> step_runs_on({}, {"platform": "linux"})
    True
    """
    condition = str(step.get("if", ""))
    if not condition or "||" in condition:
        return True
    for key, operator, value in _MATRIX_COMPARISON.findall(condition):
        actual = cell.get(key)
        if actual is None:
            continue
        if (operator == "==") != (actual == value):
            return False
    return True


def _make_goals(run: str) -> list[list[str]]:
    """Return the goal list of each ``make`` invocation in a ``run`` script."""
    invocations: list[list[str]] = []
    for line in run.splitlines():
        try:
            tokens = shlex.split(line, comments=True)
        except ValueError:
            continue
        for index, token in enumerate(tokens):
            if token == "make":
                goals = [
                    word for word in tokens[index + 1 :]
                    if not word.startswith("-") and "=" not in word
                ]
                invocations.append(goals)
    return invocations


def reaches_the_suite(step: dict[str, object], leg: Leg) -> bool:
    """Report whether a step runs tests, or reaches the build-tools preflight.

    Parameters
    ----------
    step : dict
        The parsed step.
    leg : Leg
        The leg it runs in; its job and step ``env`` can switch a Make route.

    Returns
    -------
    bool
        True for the coverage action, a direct test invocation, or a ``make``
        goal whose dry run reaches the preflight script.
    """
    if _GENERATE_COVERAGE in str(step.get("uses", "")):
        return True
    run = str(step.get("run", ""))
    if _DIRECT_SUITE.search(run):
        return True
    env = {**_env(leg.job), **_env(step)}
    switches = [f"{name}={env[name]}" for name in MAKE_SWITCHES if name in env]
    return any(
        PREFLIGHT_SCRIPT in dry_run(goal, *switches)
        for goals in _make_goals(run)
        for goal in goals
    )


def _env(mapping: dict[str, object]) -> dict[str, str]:
    """Return a mapping's ``env`` block as strings, empty when absent."""
    env = mapping.get("env")
    return {str(k): str(v) for k, v in env.items()} if isinstance(env, dict) else {}
