"""Reading a profile's override entries in nextest file order.

This module owns the parsed shape of one override and the repository-specific
reader that extracts a profile's own entries. It does not interpret filters
or resolve effective allowances; contracts needing ordered entries should use
this reader, while contracts needing nextest's per-setting precedence should
use :mod:`nextest_precedence`.
"""

from __future__ import annotations

import typing as typ

from nextest_document import budget_tables

__all__ = ["Override", "overrides_in_order"]


class Override(typ.NamedTuple):
    """One entry in a profile's override list, as nextest applies it.

    Attributes
    ----------
    path : str
        The entry's dotted path, for messages and for reporting which entry
        supplied an allowance.
    expression : str or None
        Its ``filter`` value, or ``None`` for an entry selecting without a
        filterset -- one gated on ``platform``, for instance.
    entry : dict
        The parsed table, so a caller can read the settings it configures.
    """

    path: str
    expression: str | None
    entry: dict[str, object]

    @property
    def is_platform_gated(self) -> bool:
        """Whether applying this entry needs a platform decision.

        An entry can carry *both* a ``filter`` and a ``platform`` key, and the
        gate then narrows the filter rather than replacing it. Reading the
        filter and ignoring the gate would apply a Windows-only entry to a
        Linux run -- the direction that reports a shadowed binary as raised.
        """
        return "platform" in self.entry


def overrides_in_order(config_text: str, profile: str = "default") -> list[Override]:
    """Return one profile's overrides in the order nextest applies them.

    Only the named profile's own list is read. An ``[[overrides]]`` entry
    under another profile does not apply here, and reading several profiles
    together is how coverage comes to be claimed by an entry the run never
    consults.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.
    profile : str
        The profile whose override list to read.

    Returns
    -------
    list of Override
        Each entry in declaration order. The expression is ``None`` for an
        entry that selects without a filterset, such as one gated on
        ``platform``.

    Examples
    --------
    >>> overrides_in_order(
    ...     '[profile.default]\\nslow-timeout = "60s"\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(a)"\\n'
    ... )
    [Override(path='profile.default.overrides[0]', expression='binary(a)', entry={'filter': 'binary(a)'})]
    >>> overrides_in_order(
    ...     '[profile.default]\\nslow-timeout = "60s"\\n'
    ...     '[[profile.ci]]\\nslow-timeout = "30s"\\n'
    ... )
    []
    """
    prefix = f"profile.{profile}.overrides["
    return [
        Override(
            path=path,
            expression=entry["filter"] if isinstance(entry.get("filter"), str) else None,
            entry=entry,
        )
        for path, entry in budget_tables(config_text)
        if path.startswith(prefix)
    ]
