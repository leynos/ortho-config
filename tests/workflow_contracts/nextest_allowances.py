"""Reading one declared ``slow-timeout`` as the per-test allowance it gives.

Separated from ``nextest_budgets`` so the deserialization of a single
declared value and the tiers a whole file yields stay legible apart, and
so neither module outgrows the 400-line limit ``AGENTS.md`` sets.

Everything here refuses exactly what nextest refuses. nextest reads
``terminate-after`` into an ``Option<NonZeroUsize>`` and every duration
with ``humantime_serde``, so a boolean, a float, a quoted number, zero
and a negative integer each make it refuse the file. A reader more
permissive than the runner reports a per-test tier for a configuration
that cannot run, which is the failure this module exists to prevent.
"""

from __future__ import annotations

from typing import TypeGuard

from nextest_durations import seconds
from nextest_errors import (
    NextestConfigurationError,
    UnboundedTestError,
)


def budget_of(path: str, value: object) -> float:
    """Return the per-test budget one ``slow-timeout`` declares, or raise.

    Parameters
    ----------
    path : str
        The dotted path of the declaring table, for the message.
    value : object
        The parsed value of that ``slow-timeout``.

    Returns
    -------
    float
        ``period`` multiplied by ``terminate-after``, in seconds.

    Raises
    ------
    UnboundedTestError
        If the value declares no ``terminate-after``, in either spelling,
        so nextest marks the test slow and lets it run on.
    NextestConfigurationError
        If the value is neither a table nor a duration, or names no
        ``period``.

    Examples
    --------
    >>> budget_of("profile.default", {"period": "60s", "terminate-after": 10})
    600.0
    """
    # A value naming no `terminate-after`, in either spelling, raises
    # `UnboundedTestError`: nextest marks the test slow and lets it run
    # on, so there is no per-test tier to compare against. A table with
    # no `period`, or a value that is neither a table nor a duration,
    # raises `NextestConfigurationError`.
    match value:
        case str():
            message = (
                f'{path}.slow-timeout = "{value}" sets a warning period with '
                f"no terminate-after, so nextest reports the test as slow and "
                f"never stops it"
            )
            raise UnboundedTestError(message)
        case dict():
            pass
        case _:
            message = f"{path}.slow-timeout is neither a table nor a duration"
            raise NextestConfigurationError(message)
    period = value.get("period")
    if not isinstance(period, str):
        message = f"{path}.slow-timeout names no period: {value!r}"
        raise NextestConfigurationError(message)
    multiplier = value.get("terminate-after")
    if multiplier is None:
        message = (
            f"{path}.slow-timeout sets no terminate-after, so nextest marks "
            f"the test slow and lets it run on; there is no per-test tier to "
            f"compare against"
        )
        raise UnboundedTestError(message)
    return seconds(period) * terminate_after(path, multiplier)


def is_positive_integer(value: object) -> TypeGuard[int]:
    """Return whether a parsed value is a TOML positive integer.

    Matched rather than tested with a chained condition, so each shape
    is answered on its own line. ``bool`` is answered first because it
    is a subclass of ``int`` in Python and is not one in TOML: without
    its own arm, ``terminate-after = true`` reads as a multiplier of
    one.

    Narrowing is declared with ``TypeGuard`` rather than ``TypeIs``
    because the two say different things and only the looser one is
    true here. ``TypeIs`` asserts the predicate answers True for every
    ``int``; this one refuses ``True`` and refuses zero and negatives,
    all of which are ``int``. ``TypeGuard`` asserts only that a True
    answer implies the type, which is what ``terminate_after`` needs
    to return its ``object`` argument as an ``int``.

    Parameters
    ----------
    value : object
        The parsed value.

    Returns
    -------
    TypeGuard[int]
        True when nextest would accept it as a ``NonZeroUsize``,
        narrowing the argument to ``int`` for the caller.

    Examples
    --------
    >>> is_positive_integer(2)
    True
    >>> is_positive_integer(True)
    False
    >>> is_positive_integer(1.5)
    False
    """
    match value:
        case bool():
            return False
        case int():
            return value >= 1
        case _:
            return False


def terminate_after(path: str, value: object) -> int:
    """Return a ``terminate-after`` as nextest deserializes one.

    nextest reads this field into an ``Option<NonZeroUsize>``, so it is
    a positive integer and nothing else. Reading it through
    ``float(str(...))`` accepted three shapes the runner refuses and
    misread a fourth: a TOML float such as ``1.5``, a quoted ``"2"``,
    and zero or a negative integer all became budgets, and a boolean
    raised ``ValueError`` out of this module rather than the
    configuration error every caller here handles.

    The refusal is by shape rather than by catching the conversion's
    exception. Wrapping ``float`` would report the boolean properly and
    still accept ``1.5``, ``"2"`` and zero, which is the larger half of
    the defect: a contract that multiplies a period by a multiplier
    nextest will not load reports a per-test tier for a file that
    cannot run.

    Parameters
    ----------
    path : str
        The dotted path of the declaring table, for the message.
    value : object
        The parsed value.

    Returns
    -------
    int
        The multiplier.

    Raises
    ------
    NextestConfigurationError
        If the value is not a positive integer.

    Examples
    --------
    >>> terminate_after("profile.default", 3)
    3
    """
    if not is_positive_integer(value):
        message = (
            f"{path}.slow-timeout sets terminate-after = {value!r}; nextest "
            f"reads it as a positive integer and refuses the file otherwise, "
            f"so no budget can be derived from it"
        )
        raise NextestConfigurationError(message)
    return value
