"""Locating the tables `.config/nextest.toml` declares a budget in.

Separated from ``nextest_budgets`` so the parsing of the document and the
values read out of it stay legible apart, and so neither module outgrows
the 400-line limit ``AGENTS.md`` sets.

The configuration is parsed with ``tomllib`` rather than matched as
text. A text match finds a key inside a comment, inside a ``filter``
string, or in a table nextest never consults, and reports a budget the
runner does not use.

A table is paired with the dotted path that names it -- ``profile.default``
or ``profile.default.overrides[7]`` -- rather than returned alone, because
every caller has to say which table it read a value from: a failure that
cannot name the entry at fault sends the reader looking through the whole
file.
"""

from __future__ import annotations

import tomllib

from nextest_errors import NextestConfigurationError


def table(value: object) -> dict[str, object]:
    """Return a parsed value as a table, or an empty one."""
    # `tomllib` returns whatever the document said, so a configuration
    # naming a scalar where a table belongs yields nothing here rather
    # than raising several frames away, and the assertion that finds no
    # budget reports the absence.
    return dict(value) if isinstance(value, dict) else {}


def parsed(config_text: str) -> dict[str, object]:
    """Return the nextest configuration as TOML, or raise."""
    try:
        return tomllib.loads(config_text)
    except tomllib.TOMLDecodeError as error:
        message = f"the nextest configuration is not valid TOML: {error}"
        raise NextestConfigurationError(message) from error


def budget_tables(config_text: str) -> list[tuple[str, dict[str, object]]]:
    """Return every table nextest reads a per-test budget from."""
    # Each profile's own table and each of its `[[overrides]]` entries,
    # paired with the dotted path that names it so a failure can say
    # which one is at fault.
    tables: list[tuple[str, dict[str, object]]] = []
    for name, raw in table(parsed(config_text).get("profile")).items():
        profile = table(raw)
        tables.append((f"profile.{name}", profile))
        overrides = profile.get("overrides")
        entries = overrides if isinstance(overrides, list) else []
        tables.extend(
            (f"profile.{name}.overrides[{index}]", table(entry))
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
        (path, entry["slow-timeout"])
        for path, entry in budget_tables(config_text)
        if "slow-timeout" in entry
    ]
