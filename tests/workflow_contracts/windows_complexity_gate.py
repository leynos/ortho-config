"""Readers that decide what a workflow step or job actually does.

The contract in :mod:`windows_complexity_gate_test` asks whether the gate
runs, on which leg, after its prerequisites, and in a way a failure can
block a merge. Each of those questions is really about GitHub's own
semantics — a constant condition, a matrix guard, a runner expression, a
`continue-on-error` — and each is easy to answer wrongly in a way that
makes the contract pass over a gate nobody runs.

The readers live here, with their own examples, so the contract reads as
assertions about the gate rather than as parsing. They are also why the
contract stays inside the 400-line limit `AGENTS.md` sets.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ

from workflow_legs import is_linux, runs_on_linux, step_runs_on

#: A ``matrix.<key>`` reference inside a ``runs-on`` expression.
MATRIX_REFERENCE: typ.Final[re.Pattern[str]] = re.compile(r"matrix\.([A-Za-z_][\w-]*)")

#: A quoted string a comparison introduces, which names no runner.
COMPARISON_OPERAND: typ.Final[re.Pattern[str]] = re.compile(r"(?:==|!=)\s*'([^']+)'")

#: A quoted string anywhere in a ``runs-on`` expression.
QUOTED_LITERAL: typ.Final[re.Pattern[str]] = re.compile(r"'([^']+)'")


#: The conditions GitHub evaluates to a constant false, as written. A YAML
#: ``if:`` is an expression, so ``0``, ``false``, ``null`` and the empty string
#: all skip the work; the braced and bare spellings are both accepted for the
#: ones that can appear either way.
DISABLED_CONDITIONS: typ.Final[frozenset[str]] = frozenset(
    {
        "false",
        "0",
        "null",
        '""',
        "''",
        "${{ false }}",
        "${{ 0 }}",
        "${{ null }}",
    }
)


def is_disabled(entry: dict[str, object]) -> bool:
    """Report whether a step or job can never run.

    A step or job carrying ``if: false`` is accepted by ``workflow_reading``
    like any other, so one could satisfy every assertion below while running
    no gate at all. An absent condition is not a disabled one: that is the
    ordinary case, and it stays enabled.

    Parameters
    ----------
    entry : dict
        A parsed workflow step or job.

    Returns
    -------
    bool
        Whether it is unconditionally skipped.

    Examples
    --------
    >>> is_disabled({"run": "make lint"})
    False
    >>> is_disabled({"run": "make lint", "if": False})
    True
    >>> is_disabled({"run": "make lint", "if": 0})
    True
    >>> is_disabled({"run": "make lint", "if": "${{ false }}"})
    True
    >>> is_disabled({"run": "make lint", "if": "${{ null }}"})
    True
    >>> is_disabled({"run": "make lint", "if": "${{ matrix.platform == 'linux' }}"})
    False
    """
    condition = entry.get("if")
    if condition is None:
        return False
    if isinstance(condition, bool):
        return not condition
    return str(condition).strip().lower() in DISABLED_CONDITIONS


def _run_steps(job: dict[str, object]) -> list[str]:
    """Return the `run:` bodies of one job's enabled steps.

    Parameters
    ----------
    job : dict
        A parsed workflow job.

    Returns
    -------
    list of str
        Every shell body the job runs, in declaration order.
    """
    return [
        str(step["run"])
        for step in (job.get("steps") or [])
        if isinstance(step, dict) and "run" in step and not is_disabled(step)
    ]


def _step_dicts(job: dict[str, object]) -> list[dict[str, object]]:
    """Return one job's steps that are mappings.

    Parameters
    ----------
    job : dict
        A parsed workflow job.

    Returns
    -------
    list of dict
        The steps, in declaration order.
    """
    return [step for step in (job.get("steps") or []) if isinstance(step, dict)]


def _runs(
    step: dict[str, object], cell: dict[str, str]
) -> bool:
    """Report whether *step* runs on the leg with matrix *cell*.

    `is_disabled` recognises only constant conditions. A condition that tests
    the matrix, such as `matrix.platform == 'windows'`, excludes the step from
    some legs and keeps it on others, and which is which depends on the job's
    matrix. Evaluating it per leg is what stops a gate job from counting an
    install that only a leg the pass never reaches would run.

    Parameters
    ----------
    step : dict
        The step to evaluate.
    cell : dict
        The matrix values of one leg.

    Returns
    -------
    bool
        Whether the step runs on that leg.
    """
    return not is_disabled(step) and step_runs_on(step, cell)


#: A ``matrix.<key>`` reference inside a ``runs-on`` expression.
MATRIX_REFERENCE: typ.Final[re.Pattern[str]] = re.compile(r"matrix\.([A-Za-z_][\w-]*)")

#: A quoted string a comparison introduces, which names no runner.
COMPARISON_OPERAND: typ.Final[re.Pattern[str]] = re.compile(r"(?:==|!=)\s*'([^']+)'")

#: A quoted string anywhere in a ``runs-on`` expression.
QUOTED_LITERAL: typ.Final[re.Pattern[str]] = re.compile(r"'([^']+)'")


def _runner_alternatives(label: str) -> list[str]:
    """Return the quoted runner labels an expression can select.

    A quoted string in a `runs-on` expression is either an alternative runner
    or a comparison operand. `matrix.runner || 'windows-latest'` names a
    runner; `github.event_name == 'pull_request'` names an event. Only the
    second is exempt, so a literal is skipped exactly when a comparison
    operator introduces it.

    Parameters
    ----------
    label : str
        One `runs-on` entry, possibly an expression.

    Returns
    -------
    list of str
        The quoted labels that are runner alternatives.

    Examples
    --------
    >>> _runner_alternatives("${{ matrix.runner || 'windows-latest' }}")
    ['windows-latest']
    >>> _runner_alternatives("${{ github.event_name == 'pull_request' && matrix.runner }}")
    []
    >>> _runner_alternatives("ubuntu-latest")
    []
    """
    operands = set(COMPARISON_OPERAND.findall(label))
    return [literal for literal in QUOTED_LITERAL.findall(label) if literal not in operands]


def _leg_is_linux(leg: object) -> bool:
    """Report whether every runner a leg's `runs-on` can choose is Linux.

    `workflow_legs.runs_on_linux` answers whether *any* alternative is Linux,
    which is right for asking whether a job may run there and wrong for
    asking whether it can only run there. A `runs-on` expression chooses
    between its matrix arms and any quoted alternative it names, so all of
    them are checked; a `runs-on` with no expression is a single label and is
    checked as one.

    Parameters
    ----------
    leg : workflow_legs.Leg
        The leg to judge.

    Returns
    -------
    bool
        Whether the leg can only land on Linux.
    """
    runs_on = str(leg.job.get("runs-on", ""))
    keys = MATRIX_REFERENCE.findall(runs_on)
    alternatives = _runner_alternatives(runs_on)
    if not keys and not alternatives:
        return runs_on_linux(leg)
    matrix_arms = [leg.cell[key] for key in keys if key in leg.cell]
    return all(is_linux(label) for label in [*matrix_arms, *alternatives])


def _first_running(
    steps: typ.Sequence[dict[str, object]], target: str, cell: dict[str, str]
) -> int | None:
    """Return the index of the first step running *target* on one leg.

    Parameters
    ----------
    steps : sequence of dict
        The job's steps, in declaration order, which is the order GitHub runs
        them in.
    target : str
        The Make target the step must invoke.
    cell : dict
        The matrix values of the leg being judged.

    Returns
    -------
    int or None
        The step's index, or ``None`` when no step runs that target there.
    """
    for index, step in enumerate(steps):
        if f"make {target}" in str(step.get("run", "")) and _runs(step, cell):
            return index
    return None



def _is_failure_tolerant(entry: dict[str, object]) -> bool:
    """Report whether a job or step lets a failure pass.

    `continue-on-error` may be a boolean or an expression; any value that is
    not a definite false is treated as tolerance, because the safe reading
    for a contract that demands an enforcing gate is that an unreadable
    setting might excuse the failure.

    Parameters
    ----------
    entry : dict
        A parsed workflow job or step.

    Returns
    -------
    bool
        Whether the entry can swallow a failure.

    Examples
    --------
    >>> _is_failure_tolerant({"run": "make lint"})
    False
    >>> _is_failure_tolerant({"run": "make lint", "continue-on-error": True})
    True
    >>> _is_failure_tolerant({"run": "make lint", "continue-on-error": False})
    False
    >>> _is_failure_tolerant({"continue-on-error": "${{ matrix.advisory }}"})
    True
    """
    setting = entry.get("continue-on-error")
    if setting is None:
        return False
    return setting is not False
