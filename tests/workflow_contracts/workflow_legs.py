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
  the preflight". The Make variables that change the answer come from the job
  and step ``env`` and from the invocation's own ``NAME=value`` words, in that
  order of precedence.

Every reading over-approximates reachability. A step whose condition it cannot
evaluate is treated as running, because the safe error is demanding an
installer that was not needed, not excusing one that was.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import dataclasses
import itertools
import re
import typing as typ

from build_standard_make import MakeDryRunError, PREFLIGHT_SCRIPT, dry_run
from workflow_shell import make_invocations

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
#: One whole clause of a condition: nothing may surround it but whitespace.
_MATRIX_TERM: typ.Final[re.Pattern[str]] = re.compile(
    r"\s*matrix\.([A-Za-z0-9_-]+)\s*(==|!=)\s*'([^']*)'\s*"
)
type MakeRunner = typ.Callable[[str, list[str]], list[str]]


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

    The cells are the Cartesian product of the list-valued axes, minus any cell
    an ``exclude`` entry matches. Each ``include`` entry is then added to every
    surviving cell whose axis values it does not change; an entry that matches
    no surviving cell becomes a cell of its own.

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
    cells = _matrix_cells(matrix) if isinstance(matrix, dict) else []
    return [Leg(workflow, job_id, cell, job) for cell in cells] or [Leg(workflow, job_id, {}, job)]


def _matrix_cells(matrix: dict[str, object]) -> list[dict[str, str]]:
    """Return a matrix's cells, applying GitHub's ``include`` augmentation.

    An ``include`` entry augments: its key/value pairs are added to every
    surviving product cell whose axis values the entry keeps, and the entry
    becomes a cell of its own only when no survivor matches. Two consequences
    the reader must keep straight:

    - A key that is not an axis adds a value; a key that is an axis may only
      repeat the value the cell already has, because the product is never
      overwritten. That is why an entry naming a value outside the axis
      produces a new cell rather than changing an existing one.
    - A cell this function creates is not eligible for later augmentation, so
      two entries naming the same new axis value stay two cells.
    """
    axes = _axes(matrix)
    exclusions = _entries(matrix.get("exclude"))
    product = [cell for cell in _product_cells(matrix) if not _is_excluded(cell, exclusions)]
    added: list[dict[str, str]] = []
    for entry in _entries(matrix.get("include")):
        matching = [cell for cell in product if _augments(cell, entry, axes)]
        if not matching:
            added.append(dict(entry))
            continue
        for cell in matching:
            cell.update(entry)
    return product + added


def _augments(cell: dict[str, str], entry: dict[str, str], axes: dict[str, object]) -> bool:
    """Report whether *entry* can be added to *cell* without changing an axis value."""
    return all(cell.get(key) == value for key, value in entry.items() if key in axes)


def _axes(matrix: dict[str, object]) -> dict[str, object]:
    """Return a matrix's axes: the list-valued keys other than include and exclude."""
    return {
        key: value
        for key, value in matrix.items()
        if key not in {"include", "exclude"} and isinstance(value, list)
    }


def _product_cells(matrix: dict[str, object]) -> list[dict[str, str]]:
    """Return the Cartesian product of a matrix's list-valued axes (none: no cells)."""
    axes = _axes(matrix)
    if not axes:
        return []
    return [
        {key: str(value) for key, value in zip(axes, combination, strict=True)}
        for combination in itertools.product(*axes.values())
    ]


def _entries(value: object) -> list[dict[str, str]]:
    """Return the mapping entries of an ``include`` or ``exclude`` list, as strings."""
    if not isinstance(value, list):
        return []
    return [
        {str(key): str(item) for key, item in entry.items()}
        for entry in value
        if isinstance(entry, dict)
    ]


def _is_excluded(cell: dict[str, str], exclusions: list[dict[str, str]]) -> bool:
    """Report whether any exclusion's keys all match *cell* (an exclusion may be partial)."""
    return any(
        all(cell.get(key) == value for key, value in exclusion.items())
        for exclusion in exclusions
    )


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
    raw = _raw_labels(leg)
    return [label for entry in raw for label in _resolve_label(entry, leg)]


def _raw_labels(leg: Leg) -> list[str]:
    """Return ``runs-on`` as a list of labels, whichever of the three forms it takes."""
    runs_on = leg.job.get("runs-on")
    if isinstance(runs_on, str):
        return [runs_on]
    if isinstance(runs_on, list):
        return [str(label) for label in runs_on]
    if isinstance(runs_on, dict):
        labels = runs_on.get("labels", [])
        named = [labels] if isinstance(labels, str) else [str(label) for label in labels]
        return named + ([str(runs_on["group"])] if "group" in runs_on else [])
    message = f"{leg.name} declares no readable runs-on: {runs_on!r}"
    raise UnreadableRunnerError(message)


def _resolve_label(label: str, leg: Leg) -> list[str]:
    """Return the labels one ``runs-on`` entry can name, expanding an expression."""
    if "${{" not in label:
        return [label]
    keys = _MATRIX_REFERENCE.findall(label)
    literals = _QUOTED_LITERAL.findall(label)
    if _names_no_runner(label, keys, literals):
        message = f"{leg.name} runs-on cannot be resolved to labels: {label!r}"
        raise UnreadableRunnerError(message)
    missing = [key for key in keys if key not in leg.cell]
    if missing:
        message = f"{leg.name} runs-on names matrix keys the leg lacks: {missing}"
        raise UnreadableRunnerError(message)
    return [leg.cell[key] for key in keys] + literals


def _names_no_runner(label: str, keys: list[str], literals: list[str]) -> bool:
    """Report whether an expression cannot be read as a set of runner labels.

    ``github.*`` can only be a condition (the fork fallback tests the event), so
    it is no reason to refuse; ``inputs``, ``vars`` and the like can choose a
    runner yet cannot be known from the workflow text, so they are.
    """
    has_candidates = bool(keys or literals)
    has_unknowable = _UNRESOLVABLE.search(_MATRIX_REFERENCE.sub("", label)) is not None
    return has_unknowable or not has_candidates


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

    Only a conjunction of ``matrix.<key> ==|!= '<value>'`` tests is evaluated.
    Any other condition (``always()``, ``runner.environment``, an ``||``, a
    comparison against something besides a matrix key) is treated as true: the
    safe reading for a contract that demands installers. Reading a condition
    partially would be worse than not reading it, because the comparison
    extracted from ``always() && matrix.platform == 'linux'`` would excuse a
    step whose guard also depends on an event this reader cannot see.

    Examples
    --------
    >>> step_runs_on({"if": "${{ matrix.platform == 'linux' }}"}, {"platform": "windows"})
    False
    >>> step_runs_on({"if": "${{ always() && matrix.platform != 'linux' }}"}, {"platform": "windows"})
    True
    >>> step_runs_on({"if": "${{ always() && matrix.platform == 'linux' }}"}, {"platform": "windows"})
    True
    >>> step_runs_on({}, {"platform": "linux"})
    True
    """
    terms = _condition_terms(str(step.get("if", "")))
    if terms is None:
        return True
    return all(_comparison_holds(cell.get(key), operator, value) for key, operator, value in terms)


def _condition_terms(condition: str) -> list[tuple[str, str, str]] | None:
    """Return a condition's matrix tests, or ``None`` when it is not all of them.

    ``None`` means "cannot be evaluated", which :func:`step_runs_on` reads as
    running. An absent or blank condition is not a comparison that holds; it is
    a step with no guard, and it always runs.
    """
    text = condition.strip()
    if text.startswith("${{") and text.endswith("}}"):
        text = text[3:-2]
    if not text.strip():
        return None
    terms: list[tuple[str, str, str]] = []
    for clause in text.split("&&"):
        match = _MATRIX_TERM.fullmatch(clause)
        if match is None:
            return None
        key, operator, value = match.groups()
        terms.append((key, operator, value))
    return terms


def _comparison_holds(actual: str | None, operator: str, value: str) -> bool:
    """Evaluate one ``matrix.<key> ==|!= 'value'`` test; an unknown key holds.

    GitHub compares strings without regard to case, so ``'Linux'`` and
    ``'linux'`` are the same value here too.
    """
    if actual is None:
        return True
    matches = actual.casefold() == value.casefold()
    return (operator == "==") == matches


def _make_dry_run(target: str, switches: list[str]) -> list[str]:
    """Run Make's narrow dry-run boundary for one goal and its switches."""
    return dry_run(target, *switches)


def reaches_the_suite(
    step: dict[str, object], leg: Leg, *, make_runner: MakeRunner = _make_dry_run
) -> bool:
    """Report whether a step runs tests, or reaches the build-tools preflight.

    Parameters
    ----------
    step : dict
        The parsed step.
    leg : Leg
        The leg it runs in; its job and step ``env`` can switch a Make route.
    make_runner : MakeRunner
        Narrow Make-output provider. The default executes the dry-run boundary;
        tests can supply precomputed lines without starting a subprocess.

    Returns
    -------
    bool
        True for the coverage action, a direct test invocation, or a ``make``
        goal whose dry run reaches the preflight script.

    Raises
    ------
    MakeDryRunError
        If the default Make runner cannot resolve, start, or complete Make.

    Examples
    --------
    >>> step = {"run": "make test"}
    >>> leg = Leg("ci.yml", "build", {}, {})
    >>> reaches_the_suite(step, leg, make_runner=lambda _goal, _switches: [PREFLIGHT_SCRIPT])
    True
    """
    if _GENERATE_COVERAGE in str(step.get("uses", "")):
        return True
    run = str(step.get("run", ""))
    if _DIRECT_SUITE.search(run):
        return True
    env = {**_env(leg.job), **_env(step)}
    return any(
        PREFLIGHT_SCRIPT in make_runner(goal, _switches(env, invocation.assignments))
        for invocation in make_invocations(run)
        for goal in invocation.goals
    )


def _switches(env: dict[str, str], assignments: dict[str, str]) -> list[str]:
    """Return the Make variables that change a route, in Make's precedence order.

    A command-line assignment outranks the environment and the Makefile, so it
    is applied last: ``make test BUILD_STANDARD=0`` turns the standard off for
    that invocation even where the job exports it as 1.
    """
    values = {name: env[name] for name in MAKE_SWITCHES if name in env}
    values |= {name: value for name, value in assignments.items() if name in MAKE_SWITCHES}
    return [f"{name}={values[name]}" for name in MAKE_SWITCHES if name in values]


def _env(mapping: dict[str, object]) -> dict[str, str]:
    """Return a mapping's ``env`` block as strings, empty when absent."""
    env = mapping.get("env")
    return {str(k): str(v) for k, v in env.items()} if isinstance(env, dict) else {}
