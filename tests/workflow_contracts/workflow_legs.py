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
_EXPRESSION: typ.Final[re.Pattern[str]] = re.compile(r"\$\{\{.*?\}\}")
_QUOTED_LITERAL: typ.Final[re.Pattern[str]] = re.compile(r"'([^']+)'")
_SHELL_CONTROL_MARKER: typ.Final[str] = "\ufdd0"
#: References that can choose a runner but cannot be known from the workflow text.
_UNRESOLVABLE: typ.Final[re.Pattern[str]] = re.compile(r"\b(?:inputs|vars|env|needs|steps|secrets)\.")
_MATRIX_COMPARISON: typ.Final[re.Pattern[str]] = re.compile(
    r"matrix\.([A-Za-z0-9_-]+)\s*(==|!=)\s*'([^']*)'"
)
_SHELL_CONTROL_CHARS: typ.Final[str] = ";|&()"


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
    an ``exclude`` entry matches, plus one cell per ``include`` entry.

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
    """Return a matrix's cells: the surviving product cells, then the includes."""
    exclusions = _entries(matrix.get("exclude"))
    product = [cell for cell in _product_cells(matrix) if not _is_excluded(cell, exclusions)]
    return product + _entries(matrix.get("include"))


def _product_cells(matrix: dict[str, object]) -> list[dict[str, str]]:
    """Return the Cartesian product of a matrix's list-valued axes (none: no cells)."""
    axes = {
        key: value
        for key, value in matrix.items()
        if key not in {"include", "exclude"} and isinstance(value, list)
    }
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
    return all(
        _comparison_holds(cell.get(key), operator, value)
        for key, operator, value in _MATRIX_COMPARISON.findall(condition)
    )


def _comparison_holds(actual: str | None, operator: str, value: str) -> bool:
    """Evaluate one ``matrix.<key> ==|!= 'value'`` test; an unknown key holds."""
    if actual is None:
        return True
    return (operator == "==") == (actual == value)


def _make_goals(run: str) -> list[list[str]]:
    """Return the goal list of each ``make`` invocation in a ``run`` script."""
    source = _EXPRESSION.sub("EXPR", run)
    tokenised = (_tokens(line) for line in _logical_lines(source))
    return [
        _goals_after(tokens, index)
        for tokens in tokenised
        for index, token in enumerate(tokens)
        if token == "make"
    ]


def _logical_lines(run: str) -> list[str]:
    """Join shell backslash continuations without splitting inside quotes."""
    lines: list[str] = []
    current: list[str] = []
    quote: str | None = None
    at_word_start = True
    index = 0
    while index < len(run):
        char = run[index]
        if char == "\\" and quote != "'" and index + 1 < len(run):
            newline_length = 2 if run.startswith("\r\n", index + 1) else int(run[index + 1] == "\n")
            if newline_length:
                index += 1 + newline_length
                continue
            current.extend((char, run[index + 1]))
            at_word_start = False
            index += 2
            continue
        if quote is None and char == "#" and at_word_start:
            end = run.find("\n", index)
            if end == -1:
                current.extend(run[index:])
                break
            current.extend(run[index:end])
            lines.append("".join(current))
            current.clear()
            at_word_start = True
            index = end + 1
            continue
        if quote is None and char in _SHELL_CONTROL_CHARS:
            end = index + 1
            while end < len(run) and run[end] in _SHELL_CONTROL_CHARS:
                end += 1
            current.extend((" ", _SHELL_CONTROL_MARKER, " "))
            at_word_start = True
            index = end
            continue
        if quote is None and char in {'"', "'"}:
            quote = char
        elif quote == char:
            quote = None
        if char == "\n" and quote is None:
            lines.append("".join(current))
            current.clear()
            at_word_start = True
            index += 1
            continue
        current.append(char)
        at_word_start = char.isspace() or char in _SHELL_CONTROL_CHARS
        index += 1
    if current:
        lines.append("".join(current))
    return lines


def _tokens(line: str) -> list[str]:
    """Split one script line into words, or return none for a line shell cannot parse.

    An expression such as `MSRV=${{ matrix.toolchain }}` holds spaces, which
    would split into stray words that read as goals; GitHub resolves it before
    the shell runs, so it is replaced by a placeholder word. Operators are
    marked as control boundaries.
    """
    try:
        return shlex.split(_EXPRESSION.sub("EXPR", line), comments=True)
    except ValueError:
        return []


def _goals_after(tokens: list[str], index: int) -> list[str]:
    """Return Make goals up to the next shell operator, omitting flags/assignments."""
    goals: list[str] = []
    for word in tokens[index + 1 :]:
        if word == _SHELL_CONTROL_MARKER:
            break
        if not word.startswith("-") and "=" not in word:
            goals.append(word)
    return goals


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
