"""Reading `.config/nextest.toml` as the runner reads it.

Separated from ``timeout_budgets`` so the configuration reading and the
values it is compared against stay legible apart, and so neither module
outgrows the 400-line limit ``AGENTS.md`` sets. Reading one declared
value as a number of seconds lives in ``nextest_allowances`` for the
same reason.

The configuration is parsed with ``tomllib`` rather than matched as
text. A text match finds a key inside a comment, inside a ``filter``
string, or in a table nextest never consults, and reports a budget the
runner does not use.
"""

from __future__ import annotations

import tomllib
from itertools import starmap

from nextest_allowances import budget_of, is_positive_integer
from nextest_durations import seconds
from nextest_errors import (
    NextestConfigurationError,
    TimeoutBudgetError,
    UnboundedTestError,
)
from timeout_budgets import (
    NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS,
    TERMINATION_SAFETY_MARGIN_SECONDS,
)

#: Re-exported so every module that reads the configuration through this
#: one keeps a single import site for the faults it reports and for the
#: duration reading those faults come out of.
__all__ = [
    "NextestConfigurationError",
    "TimeoutBudgetError",
    "UnboundedTestError",
    "seconds",
]


def _table(value: object) -> dict[str, object]:
    """Return a parsed value as a table, or an empty one."""
    # `tomllib` returns whatever the document said, so a configuration
    # naming a scalar where a table belongs yields nothing here rather
    # than raising several frames away, and the assertion that finds no
    # budget reports the absence.
    return dict(value) if isinstance(value, dict) else {}


def _parsed(config_text: str) -> dict[str, object]:
    """Return the nextest configuration as TOML, or raise."""
    try:
        return tomllib.loads(config_text)
    except tomllib.TOMLDecodeError as error:
        message = f"the nextest configuration is not valid TOML: {error}"
        raise NextestConfigurationError(message) from error


def _budget_tables(config_text: str) -> list[tuple[str, dict[str, object]]]:
    """Return every table nextest reads a per-test budget from."""
    # Each profile's own table and each of its `[[overrides]]` entries,
    # paired with the dotted path that names it so a failure can say
    # which one is at fault.
    tables: list[tuple[str, dict[str, object]]] = []
    for name, raw in _table(_parsed(config_text).get("profile")).items():
        profile = _table(raw)
        tables.append((f"profile.{name}", profile))
        overrides = profile.get("overrides")
        entries = overrides if isinstance(overrides, list) else []
        tables.extend(
            (f"profile.{name}.overrides[{index}]", _table(entry))
            for index, entry in enumerate(entries)
        )
    return tables


def slow_timeouts(config_text: str) -> list[tuple[str, object]]:
    """Return each ``slow-timeout`` with the path of the table declaring it.

    The path is the dotted location nextest reads that table from, such as
    ``profile.default`` or ``profile.default.overrides[0]``, so a caller
    can tell a profile's own allowance from an override's.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    list of (str, object)
        Each declared ``slow-timeout`` and the table declaring it.

    Examples
    --------
    >>> slow_timeouts('[profile.default]\\nslow-timeout = "30s"\\n')
    [('profile.default', '30s')]
    """
    return [
        (path, table["slow-timeout"])
        for path, table in _budget_tables(config_text)
        if "slow-timeout" in table
    ]


def override_allowances(config_text: str) -> list[tuple[str, str, float]]:
    """Return each override's filter and the per-test allowance it carries.

    An override is the only way a set of tests gets an allowance other
    than the profile's own, so this is the reader that answers which
    filter a given allowance belongs to.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    list of (str, str, float)
        Each override's path, its ``filter`` expression, and its budget
        in seconds, in file order.

    An override declaring no ``slow-timeout`` is left out rather than
    reported with no allowance: it grants no per-test budget, so there
    is nothing to compare.

    Raises
    ------
    NextestConfigurationError
        If such an override declares no ``filter``, which nextest
        refuses, or its ``slow-timeout`` is unreadable.

    Examples
    --------
    >>> override_allowances(
    ...     '[profile.default]\\n[[profile.default.overrides]]\\n'
    ...     'filter = "binary(a)"\\nslow-timeout = { period = "1m", '
    ...     'terminate-after = 2 }\\n'
    ... )
    [('profile.default.overrides[0]', 'binary(a)', 120.0)]
    """
    found: list[tuple[str, str, float]] = []
    for path, table in _budget_tables(config_text):
        if "overrides[" not in path or "slow-timeout" not in table:
            continue
        expression = table.get("filter")
        if not isinstance(expression, str):
            message = (
                f"{path} declares a slow-timeout but no filter, so there is "
                f"no set of tests that allowance applies to; nextest refuses "
                f"such a file"
            )
            raise NextestConfigurationError(message)
        found.append((path, expression, budget_of(path, table["slow-timeout"])))
    return found


def configured_periods(config_text: str) -> list[float]:
    """Return every warning period nextest reads, in seconds.

    The period a ``slow-timeout`` names, whether it is a bare duration
    or the ``period`` key of a table, for each profile and each of its
    overrides. Read from the parsed document rather than matched as
    text, so a period written inside a comment, inside an override's
    ``filter`` expression, or in a table nextest never consults is not
    counted as one in force.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    list of float
        Each configured period in seconds, in file order.
    """
    periods: list[float] = []
    for _, value in slow_timeouts(config_text):
        match value:
            case str():
                periods.append(seconds(value))
            case {"period": str() as period}:
                periods.append(seconds(period))
            case _:
                continue
    return periods


def largest_test_allowance(config_text: str) -> float:
    """Return the longest a single test may run, in seconds.

    nextest warns once per ``period`` and terminates after
    ``terminate-after`` of them, so the budget is their product. This
    repository sets five on Linux and ten on Windows against a 60 s
    period, so reading the period alone would understate the largest
    allowance by a factor of ten.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The longest per-test budget.

    A ``slow-timeout`` that names no ``terminate-after`` raises
    :class:`UnboundedTestError` from
    :func:`nextest_allowances.budget_of` rather than counting as one
    period, because such a configuration bounds nothing.

    Raises
    ------
    NextestConfigurationError
        If the configuration declares no ``slow-timeout`` at all.
    """
    budgets = list(starmap(budget_of, slow_timeouts(config_text)))
    if not budgets:
        message = (
            "the nextest configuration declares no slow-timeout, so no test "
            "is bounded and there is no per-test tier to compare against"
        )
        raise NextestConfigurationError(message)
    return max(budgets)


def profile_allowance(config_text: str, profile: str = "default") -> float:
    """Return a profile's own per-test allowance, in seconds.

    Only the profile's own ``slow-timeout`` counts, not an override's. That
    is the allowance a test which no override matches gets, so it is the
    figure an override's own is compared against when asking whether the
    override buys anything.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.
    profile : str
        The profile to read.

    Returns
    -------
    float
        The profile's own per-test budget.

    Raises
    ------
    NextestConfigurationError, UnboundedTestError
        If the profile declares no readable ``slow-timeout``, under the
        same rules :func:`largest_test_allowance` applies.

    Examples
    --------
    >>> profile_allowance('[profile.default]\\nslow-timeout = { period = "60s", '
    ...                   'terminate-after = 10 }\\n')
    600.0
    """
    own = _table(_table(_parsed(config_text).get("profile")).get(profile))
    if "slow-timeout" not in own:
        message = (
            f"profile.{profile} declares no slow-timeout of its own, so there "
            f"is no per-test allowance to compare an override against"
        )
        raise NextestConfigurationError(message)
    return budget_of(f"profile.{profile}", own["slow-timeout"])


def bounds_a_single_test(config_text: str, profile: str = "default") -> bool:
    """Return whether a profile's own table terminates a slow test.

    Only the profile's own ``slow-timeout`` counts. An override bounds
    the tests its filter matches; the profile's own bounds the rest, so
    a profile whose only ``terminate-after`` sits in an override leaves
    every unmatched test running with no bound at all while
    :func:`largest_test_allowance` still reports a comfortable number.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.
    profile : str
        The profile to read.

    The value is judged by the same rule the budget derivation uses,
    not merely by being present. nextest reads ``terminate-after`` as an
    ``Option<NonZeroUsize>``, so a boolean, a quoted number, a float,
    zero and a negative integer each make it refuse the file. Reading
    any non-null value as a bound reported the profile as bounding a
    single test for a configuration that cannot run, while
    :func:`largest_test_allowance` refused the same text: the two
    readings disagreed about the same field.

    Returns
    -------
    bool
        True when that profile's own ``slow-timeout`` is a table whose
        ``terminate-after`` nextest would accept.
    """
    own = _table(_table(_parsed(config_text).get("profile")).get(profile))
    table = own.get("slow-timeout")
    if not isinstance(table, dict):
        return False
    return is_positive_integer(table.get("terminate-after"))


def grace_period(config_text: str) -> float:
    """Return the longest grace period the configuration names, in seconds.

    Read from the configuration rather than fixed, so a profile that
    raised its grace period raises the requirement too. nextest's own
    default applies when none is named, as none is here.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The largest configured grace period, or nextest's default.
    """
    periods = [
        seconds(grace)
        for _, value in slow_timeouts(config_text)
        if isinstance(value, dict)
        and isinstance(grace := value.get("grace-period"), str)
    ]
    return max(periods, default=NEXTEST_DEFAULT_GRACE_PERIOD_SECONDS)


def termination_allowance(config_text: str) -> float:
    """Return the time nextest may take to stop the run, in seconds.

    Two terms, not one. Hitting the whole-run budget starts nextest's
    ordinary termination procedure rather than stopping the run: on Unix
    it signals the process group and waits ``slow-timeout.grace-period``
    before killing it; on Windows termination is immediate and the grace
    period is ignored for timeouts. That grace period is the first term;
    the second is a fixed margin for the teardown and report writing
    that follow it. A single floor over the two would absorb every grace
    period below the margin, so raising one would look free until the
    run it cancelled.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float
        The grace period plus the safety margin.
    """
    return grace_period(config_text) + TERMINATION_SAFETY_MARGIN_SECONDS


def global_timeout(config_text: str) -> float | None:
    """Return the whole-run budget, or None when none is set.

    Read from ``[profile.default]`` alone. nextest's other profiles
    inherit that table unless they override it, and an ``[[overrides]]``
    entry cannot carry one, so a value found elsewhere is not the budget
    in force.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    float or None
        The whole-run budget in seconds, or None when the default
        profile declares none.
    """
    profile = _table(_table(_parsed(config_text).get("profile")).get("default"))
    budget = profile.get("global-timeout")
    return seconds(budget) if isinstance(budget, str) else None
