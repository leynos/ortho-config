"""Resolving which override actually supplies an allowance to a test.

A configuration that lists a binary in a raised entry has not necessarily
raised it. nextest resolves overrides as an **ordered list**: "Starting with
the selected profile, then following its inheritance chain through `default`,
the first override that matches T and configures S is applied." Precedence is
resolved "separately for each setting", so an entry setting only ``test-group``
does not shadow an earlier entry's ``slow-timeout``, while an entry setting
``slow-timeout`` to the profile's own figure does.

That is a different question from "which entries mention this binary", which a
union of above-base entries answers. The two disagree exactly when a shadowing
entry sits *above* a raised one: every raised entry still lists the binary, so
the union reports it covered, while the runner gives it the earlier entry's
allowance. A contract that unions above-base entries therefore passes a
configuration in which the class is back on the base tier.

**Scope and re-use policy.** This reads *this repository's* configuration; it
is not a nextest implementation. It resolves file order, and where it cannot
read an expression it says so per name rather than either guessing or refusing
the whole file. A third contract needing effective allowances should import
:func:`effective_allowances` rather than grow a second resolution of the same
list.

Two behaviours are deliberately *not* modelled, and both are refused rather
than approximated. Profile inheritance is not walked: only the named profile's
own override list is read, which is exact for ``default`` -- the profile these
contracts assert -- and would under-read a custom profile that inherits
``default``'s entries. Platform gates are not applied: an entry gated on
``cfg(windows)`` is recorded but not treated as selecting a name, because
nothing here knows which platform is running. The contract that uses this
asserts a configuration read for the Linux lane, which is where
``make test-workflow-contracts`` runs; a caller wanting the Windows reading
must supply that decision rather than receive it silently.

See "Test timeouts" in ``docs/developers-guide.md``.
"""

from __future__ import annotations

import typing as typ

from nextest_allowances import budget_of
from nextest_budgets import profile_allowance
from nextest_document import budget_tables
from nextest_filterset import UnreadableMatcherError, binaries_selected_by

__all__ = ["EffectiveTier", "effective_allowances", "overrides_in_order"]


class EffectiveTier(typ.NamedTuple):
    """What nextest gives one binary, and by which entry.

    Attributes
    ----------
    allowance : float
        The per-test allowance in force, in seconds.
    path : str
        The entry granting it, or the profile's own path when no override
        supplies one.
    is_above_base : bool
        Whether that allowance exceeds the profile's own. False means the
        binary runs on the base tier, whatever entries mention it.
    is_unresolved : bool
        True when an entry earlier in the file might select this binary but
        carries an expression this reader could not evaluate. The allowance
        above is then what the *readable* entries give, and may not be what
        nextest applies: a caller asserting a tier must refuse such a case
        rather than read the figure as certain.
    """

    allowance: float
    path: str
    is_above_base: bool
    is_unresolved: bool


def overrides_in_order(
    config_text: str, profile: str = "default"
) -> list[tuple[str, str | None, dict[str, object]]]:
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
    list of (str, str or None, dict)
        Each entry's path, its filter expression, and its parsed table, in
        declaration order. The expression is ``None`` for an entry that
        selects without a filterset, such as one gated on ``platform``.

    Examples
    --------
    >>> overrides_in_order(
    ...     '[profile.default]\\nslow-timeout = "60s"\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(a)"\\n'
    ... )
    [('profile.default.overrides[0]', 'binary(a)', {'filter': 'binary(a)'})]
    >>> overrides_in_order(
    ...     '[profile.default]\\nslow-timeout = "60s"\\n'
    ...     '[[profile.ci]]\\nslow-timeout = "30s"\\n'
    ... )
    []
    """
    prefix = f"profile.{profile}.overrides["
    return [
        (
            path,
            entry["filter"] if isinstance(entry.get("filter"), str) else None,
            entry,
        )
        for path, entry in budget_tables(config_text)
        if path.startswith(prefix)
    ]


def _could_name(expression: str, name: str) -> bool:
    """Report whether an expression this reader refused could select ``name``.

    Asked only after the reader has refused an expression, so the question is
    whether refusing it is *unsafe for this name*, not what it selects. The
    answer is a deliberate over-approximation: the name's text appearing
    anywhere in the expression is taken as "it might". That can flag an
    expression which in fact selects nothing by that name, costing a caller a
    refusal it did not need; the opposite error would let an unreadable entry
    shadow a raised one and report the class as correctly tiered.

    Examples
    --------
    >>> _could_name("binary_id(ortho_config::compile_fail)", "compile_fail")
    True
    >>> _could_name("binary_id(ortho_config::compile_fail)", "cli_dispatch")
    False
    """
    return name in expression


def effective_allowances(
    config_text: str,
    names: typ.Iterable[str],
    *,
    selected_by: typ.Callable[[str], typ.Mapping[str, bool]],
    refuses: type[Exception],
    profile: str = "default",
) -> dict[str, EffectiveTier]:
    """Return the tier nextest gives each name, resolving shadowing.

    The entries are walked in file order and the first that selects a name
    *and* configures ``slow-timeout`` supplies it, which is nextest's rule.
    An entry that selects the name but configures another setting does not
    shadow, because precedence is resolved per setting.

    An entry that selects through ``platform`` alone is skipped: this reader
    has no platform decision to apply, and neither skipping nor applying it
    can be shown correct here for a name that entry might select. That is
    exact for the entries this repository writes, which name the binaries
    their own filter selects.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.
    names : iterable of str
        The binary names to resolve.
    selected_by : callable
        Given a filter expression, returns which of the caller's candidates
        it selects. Passed in because the universe is the caller's -- the
        class and its complement -- and this reader must not invent one.
    refuses : type
        The exception ``selected_by`` raises for an expression it cannot
        evaluate.
    profile : str
        The profile to resolve under.

    Returns
    -------
    dict of str to EffectiveTier
        One entry per name, in the order given.

    Examples
    --------
    A base-tier entry above a raised one is what nextest applies, so the name
    resolves to the base figure rather than to the raised one below it:

    >>> config = (
    ...     '[profile.default]\\nslow-timeout = { period = "60s", '
    ...     'terminate-after = 10 }\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(a)"\\n'
    ...     'slow-timeout = { period = "60s", terminate-after = 10 }\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(a)"\\n'
    ...     'slow-timeout = { period = "120s", terminate-after = 8 }\\n'
    ... )
    >>> resolved = effective_allowances(
    ...     config,
    ...     ["a"],
    ...     selected_by=lambda expression: {"a": True},
    ...     refuses=ValueError,
    ... )
    >>> (resolved["a"].path, resolved["a"].allowance, resolved["a"].is_above_base)
    ('profile.default.overrides[0]', 600.0, False)

    Precedence is per setting, so the same entry setting only ``test-group``
    does *not* take the raised allowance away:

    >>> grouped = (
    ...     '[profile.default]\\nslow-timeout = { period = "60s", '
    ...     'terminate-after = 10 }\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(a)"\\n'
    ...     'test-group = "shared"\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(a)"\\n'
    ...     'slow-timeout = { period = "120s", terminate-after = 8 }\\n'
    ... )
    >>> resolved = effective_allowances(
    ...     grouped,
    ...     ["a"],
    ...     selected_by=lambda expression: {"a": True},
    ...     refuses=ValueError,
    ... )
    >>> (resolved["a"].path, resolved["a"].is_above_base)
    ('profile.default.overrides[1]', True)
    """
    base = profile_allowance(config_text, profile)
    entries = overrides_in_order(config_text, profile)
    resolved: dict[str, EffectiveTier] = {}
    for name in names:
        tier = EffectiveTier(
            allowance=base,
            path=f"profile.{profile}",
            is_above_base=False,
            is_unresolved=False,
        )
        for path, expression, entry in entries:
            if "slow-timeout" not in entry or expression is None:
                continue
            try:
                chosen = selected_by(expression)
            except refuses:
                # The reader refused this expression -- a shape outside the
                # model -- and it may still shadow the allowance about to be
                # attributed to a later entry. Marked rather than trusted.
                if _could_name(expression, name):
                    tier = tier._replace(is_unresolved=True)
                continue
            if chosen.get(name):
                allowance = budget_of(path, entry["slow-timeout"])
                tier = tier._replace(
                    allowance=allowance,
                    path=path,
                    is_above_base=allowance > base,
                )
                break
        resolved[name] = tier
    return resolved


def selected_by_universe(
    candidates: typ.Iterable[str],
) -> typ.Callable[[str], dict[str, bool]]:
    """Return a ``selected_by`` reader over a fixed set of candidates.

    A convenience for callers, and the one this module's own tests use, so
    the reader passed to :func:`effective_allowances` is the same reader the
    contracts use rather than a stub that always agrees with them.

    Parameters
    ----------
    candidates : iterable of str
        The binary names to resolve expressions against.

    Returns
    -------
    callable
        A function from a filter expression to each candidate's selection.

    Examples
    --------
    >>> reader = selected_by_universe(["a", "b"])
    >>> reader('binary(a)')
    {'a': True, 'b': False}
    """
    universe = list(candidates)

    def read(expression: str) -> dict[str, bool]:
        return binaries_selected_by(expression, universe)

    return read


#: Re-exported so a caller can name the exception it passes as ``refuses``
#: without importing the filter language module as well.
REFUSES: typ.Final[type[Exception]] = UnreadableMatcherError
