"""Reading a nextest duration as the runner reads one.

Separated from ``nextest_budgets`` so the grammar and the configuration
structure stay legible apart, and so neither module outgrows the
400-line limit ``AGENTS.md`` sets.

nextest deserializes every duration with ``humantime_serde``. That
grammar is wider than one number and one short unit, and a reader
narrower than it refuses configuration nextest loads, which makes the
contract fail on a correct file and blame the file for it.
"""

from __future__ import annotations

import re
import typing as typ

from nextest_errors import NextestConfigurationError

#: One value-and-unit pair of a duration as ``humantime`` spells it.
#: nextest deserializes every duration with ``humantime_serde``, which
#: reads a sequence of such pairs and sums them, so ``"2h 37m"`` and
#: ``"1m30s"`` are configuration it accepts. A reader taking one pair
#: with one short unit refuses a file nextest loads, and the contract
#: then fails on a correct file and blames the file for it.
#:
#: The grammar was measured against humantime 2.3.0, which is what the
#: lockfile of the pinned cargo-nextest release resolves (the shared
#: coverage action this repository uses installs
#: ``cargo-nextest@0.9.120``), by compiling that parser and running the
#: cases through it. The fractional part is optional and humantime
#: tolerates whitespace around the point, so ``"1.5m"`` and
#: ``"1 . 5 m"`` are both ninety seconds. A leading point, a missing
#: fractional part, a second point, a sign and a digit separator are all
#: refused there, and so are refused here.

#: Digits with whitespace tolerated between them. humantime's parser
#: skips whitespace while it accumulates a number, so ``"1 0s"`` is ten
#: seconds rather than a malformed duration, and the same holds either
#: side of the point: ``"1 2 . 3 4 s"`` is 12.34 seconds.
_SPACED_DIGITS: typ.Final[str] = r"\d(?:\s*\d)*"

_COMPONENT: typ.Final[re.Pattern[str]] = re.compile(
    # The micro sign is written as an escape: the literal is visually
    # indistinguishable from the Greek small letter mu, which humantime
    # refuses, so the two must not be told apart by eye here.
    rf"(?P<value>{_SPACED_DIGITS}(?:\s*\.\s*{_SPACED_DIGITS})?)"
    r"\s*(?P<unit>[A-Za-z\u00b5]+)\s*"
)

#: The one duration humantime reads with no unit. Its parser
#: special-cases the exact text before reading a single character, so
#: the comparison is against the raw value rather than a stripped one:
#: ``" 0 "``, ``"0 "``, ``"00"`` and ``"0.0"`` are each refused, and a
#: reader that stripped first would accept a duration nextest rejects.
_BARE_ZERO: typ.Final[str] = "0"

#: Every unit spelling ``humantime`` accepts, with its length in whole
#: nanoseconds. Nanoseconds rather than seconds because humantime works
#: in them: a component whose value is not a whole number of them is
#: refused, and reading the table in floating-point seconds could not
#: tell such a component from a representable one. Case is not folded:
#: ``m`` is minutes and ``M`` is months, so folding would read a
#: thirty-minute budget as a two-and-a-half-year one. A month is a
#: twelfth of a Julian year and a year is 365.25 days, which is how
#: ``humantime`` defines them. Spelt out in full rather than trimmed to
#: the spellings this repository happens to use, because refusing a unit
#: nextest accepts fails a configuration the runner is happy with.
_UNIT_NANOSECONDS: typ.Final[dict[str, int]] = {
    "nanos": 1,
    "nsec": 1,
    "ns": 1,
    "usec": 1000,
    "us": 1000,
    "\u00b5s": 1000,
    "millis": 1000000,
    "msec": 1000000,
    "ms": 1000000,
    "seconds": 1000000000,
    "second": 1000000000,
    "secs": 1000000000,
    "sec": 1000000000,
    "s": 1000000000,
    "minutes": 60000000000,
    "minute": 60000000000,
    "mins": 60000000000,
    "min": 60000000000,
    "m": 60000000000,
    "hours": 3600000000000,
    "hour": 3600000000000,
    "hrs": 3600000000000,
    "hr": 3600000000000,
    "h": 3600000000000,
    "days": 86400000000000,
    "day": 86400000000000,
    "d": 86400000000000,
    "weeks": 604800000000000,
    "week": 604800000000000,
    "wks": 604800000000000,
    "wk": 604800000000000,
    "w": 604800000000000,
    "months": 2630016000000000,
    "month": 2630016000000000,
    "M": 2630016000000000,
    "years": 31557600000000000,
    "year": 31557600000000000,
    "yrs": 31557600000000000,
    "yr": 31557600000000000,
    "y": 31557600000000000,
}


#: How many nanoseconds humantime counts to a second.
_NANOSECONDS_PER_SECOND: typ.Final[int] = 1_000_000_000

#: The largest value humantime will read, for a numeric literal and for
#: the accumulated seconds alike. Its parser holds both in a ``u64``,
#: so ``"18446744073709551615s"`` loads and one second more does not.
_U64_MAX: typ.Final[int] = 2**64 - 1


def _nanoseconds_of(duration: str, value: str, unit_nanos: int) -> int:
    """Return one component's length in whole nanoseconds.

    humantime accumulates a duration in nanoseconds and refuses a
    component that does not land on one: ``"0.0000000015s"`` is a second
    and a half of nanoseconds and will not load, while
    ``"1.999999999s"`` will. Reading the value through ``float`` instead
    silently rounds the first to something plausible, so the contract
    would certify a configuration nextest cannot load. The arithmetic is
    therefore exact, in integers.

    Parameters
    ----------
    duration : str
        The whole duration, for the message.
    value : str
        The component's digits, with humantime's tolerated whitespace
        already removed.
    unit_nanos : int
        The unit's length in nanoseconds.

    Returns
    -------
    int
        The component's length in whole nanoseconds.

    Raises
    ------
    NextestConfigurationError
        If the literal is larger than humantime reads, or the component
        is not a whole number of nanoseconds.
    """
    whole, _, fraction = value.partition(".")
    scale = 10 ** len(fraction)
    if int(whole) > _U64_MAX:
        message = (
            f"nextest duration {duration!r} names {whole!r}, which is larger "
            f"than the u64 humantime reads a number into"
        )
        raise NextestConfigurationError(message)
    scaled = (int(whole) * scale + int(fraction or 0)) * unit_nanos
    if scaled % scale:
        message = (
            f"nextest duration {duration!r} is not a whole number of "
            f"nanoseconds, and humantime supports no finer precision"
        )
        raise NextestConfigurationError(message)
    return scaled // scale


def _component_at(duration: str, text: str, position: int) -> tuple[int, int]:
    """Return one component's length in nanoseconds and where it ends.

    Parameters
    ----------
    duration : str
        The whole duration, carried for the message so a failure names
        what was configured rather than the tail being read.
    text : str
        The duration with its surrounding whitespace removed.
    position : int
        Where in ``text`` this component starts.

    Returns
    -------
    tuple of (int, int)
        The component's length in nanoseconds, and the offset at which
        the next component starts.

    Raises
    ------
    NextestConfigurationError
        If no component starts here, if its unit is not one humantime
        accepts, or if its value is one humantime cannot represent.
    """
    component = _COMPONENT.match(text, position)
    if component is None:
        message = (
            f"unrecognized nextest duration {duration!r}; nextest reads "
            f"durations with humantime, which wants a sequence of numbers "
            f'each carrying a unit, such as "60s", "2h 37m" or "1.5m"'
        )
        raise NextestConfigurationError(message)
    unit = component["unit"]
    unit_nanos = _UNIT_NANOSECONDS.get(unit)
    if unit_nanos is None:
        message = (
            f"nextest duration {duration!r} names the unit {unit!r}, which "
            f"humantime does not accept; note that 'm' is minutes and 'M' "
            f"is months"
        )
        raise NextestConfigurationError(message)
    # humantime tolerates whitespace inside and around the number, so
    # the matched value can read "1 . 5"; the digits are joined before
    # the arithmetic reads them.
    value = "".join(component["value"].split())
    return _nanoseconds_of(duration, value, unit_nanos), component.end()


def seconds(duration: str) -> float:
    """Convert a nextest duration to seconds.

    Parameters
    ----------
    duration : str
        A duration as nextest spells it, such as ``"60s"``, the
        multi-component ``"2h 37m"`` or the fractional ``"1.5m"``.

    Returns
    -------
    float
        The duration in seconds.

    Raises
    ------
    NextestConfigurationError
        If the text is not a duration nextest would accept.
    """
    if duration == _BARE_ZERO:
        return 0.0
    text = duration.strip()
    if not text:
        message = (
            f"unrecognized nextest duration {duration!r}: it is empty, and "
            f"humantime reads no duration from nothing"
        )
        raise NextestConfigurationError(message)
    total = 0
    position = 0
    while position < len(text):
        length, position = _component_at(duration, text, position)
        total += length
    # humantime accumulates the whole seconds in a u64 beside a
    # nanosecond remainder, so a sum past that ceiling will not load even
    # though each component did.
    if total // _NANOSECONDS_PER_SECOND > _U64_MAX:
        message = (
            f"nextest duration {duration!r} totals more seconds than the u64 "
            f"humantime accumulates them in"
        )
        raise NextestConfigurationError(message)
    return total / _NANOSECONDS_PER_SECOND
