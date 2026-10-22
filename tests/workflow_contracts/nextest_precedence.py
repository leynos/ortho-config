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
``default``'s entries. Platform gates are not applied: nothing here knows which
platform is running, so an entry carrying ``platform`` never supplies an
allowance, and a name it might select is reported unresolved rather than
resolved to a figure the run may never give it. The contract that uses this
asserts a configuration read for the Linux lane, which is where
``make test-workflow-contracts`` runs; a caller wanting the Windows reading
must supply that decision rather than receive it silently.

Narrowing the mark to the names a gate could actually claim is what keeps it
usable. A gate narrows its filter rather than replacing it, so an entry
carrying both keys is read exactly where that filter is readable: a name the
filter excludes cannot be claimed whatever the platform decides, and does not
raise the mark. Where there is nothing to read -- no expression, or one this
reader refuses -- every name remains possible, which is why the mark is a
per-name question rather than a per-file one.

A gate and a filter are independent keys, and an entry may carry both: the gate
then narrows the filter rather than replacing it. Reading such an entry's
filter and ignoring its gate applies a Windows-only entry to a Linux run, which
reports a shadowed binary as raised -- the error direction that matters. Both
shapes are therefore treated alike, which is why the gate is checked before the
expression rather than inferred from its absence.

See "Test timeouts" in ``docs/developers-guide.md``.
"""

from __future__ import annotations

import typing as typ

from nextest_allowances import budget_of
from nextest_budgets import profile_allowance
from nextest_document import budget_tables
from nextest_filterset import UnreadableMatcherError, binaries_selected_by

__all__ = [
    "EffectiveTier",
    "FilterReader",
    "effective_allowances",
    "overrides_in_order",
    "selected_by_universe",
]


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


class FilterReader(typ.NamedTuple):
    """A reader of filter expressions, with the refusal it raises.

    The callable and the exception are one value because neither is usable
    without the other. A caller holding only the callable cannot tell an
    expression the reader *refused* -- a shape outside its model -- from one
    that selects nothing, and would silently treat an unreadable entry as a
    non-matching one. That is the error this instance records rather than
    leaves to each caller to remember.

    Attributes
    ----------
    read : callable
        Maps an expression to each candidate's selection.
    refuses : type
        The exception ``read`` raises for an expression it cannot evaluate.
    """

    read: typ.Callable[[str], typ.Mapping[str, bool]]
    refuses: type[Exception]


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


def _tier_from(override: Override, base: float) -> EffectiveTier:
    """Return the tier one entry gives a name it selects.

    Examples
    --------
    A ``slow-timeout`` above the base figures as raised:

    >>> _tier_from(
    ...     Override("profile.default.overrides[1]", "binary(a)",
    ...              {"slow-timeout": {"period": "120s", "terminate-after": 8}}),
    ...     600.0,
    ... )
    EffectiveTier(allowance=960.0, path='profile.default.overrides[1]', is_above_base=True, is_unresolved=False)
    """
    allowance = budget_of(override.path, override.entry["slow-timeout"])
    return EffectiveTier(
        allowance=allowance,
        path=override.path,
        is_above_base=allowance > base,
        is_unresolved=False,
    )


def _decide(override: Override, name: str, reader: FilterReader) -> bool | None:
    """Whether an entry supplies a figure to ``name``: yes, no, or undecidable.

    Three outcomes rather than two, because the walk asks two things of the
    same entry and they need different information. An entry supplies a figure
    only when it decides *yes*. It leaves the name unresolved when the decision
    is *undecidable*, and does nothing at all on a decided *no*.

    Nothing here reports an entry as applying that this reader cannot show
    applies. A filter this reader can read is answered exactly, which is what a
    platform-gated entry needs: a gate *narrows* its filter rather than
    replacing it, so a filter that reads and excludes the name excludes it
    whatever the platform decides. Where the filter selects the name and the
    gate is present, the platform is unknown, so the answer is ``None``. Only
    where nothing at all can be read -- no expression, or one the reader
    refused -- is the answer the deliberate over-approximation of
    :func:`_could_name`.

    Examples
    --------
    A readable expression is answered from the selection, not from a substring:

    >>> reader = selected_by_universe(["cli_dispatch", "other_binary"])
    >>> _decide(
    ...     Override("profile.default.overrides[0]", "binary(cli_dispatch)", {}),
    ...     "cli_dispatch",
    ...     reader,
    ... )
    True
    >>> _decide(
    ...     Override("profile.default.overrides[0]", "binary(cli_dispatch)", {}),
    ...     "other_binary",
    ...     reader,
    ... )
    False

    A gated entry whose filter excludes the name is still a decided no, and one
    whose filter includes it is undecidable rather than applied:

    >>> gated = Override(
    ...     "profile.default.overrides[0]", "binary(cli_dispatch)", {"platform": "cfg(unix)"}
    ... )
    >>> (_decide(gated, "cli_dispatch", reader), _decide(gated, "other_binary", reader))
    (None, False)

    An entry selecting without any expression names anything it is admitted
    for, so no name can be ruled out, and a refused expression falls back to a
    substring test that ``other_binary`` passes:

    >>> _decide(Override("profile.default.overrides[0]", None, {}), "x", reader)
    >>> _decide(
    ...     Override("profile.default.overrides[0]", "binary(cli_dispatch) & test(x)", {}),
    ...     "cli_dispatch",
    ...     reader,
    ... )
    """
    if override.expression is None:
        return None
    try:
        chosen = bool(reader.read(override.expression).get(name))
    except reader.refuses:
        return None if _could_name(override.expression, name) else False
    if chosen:
        return None if override.is_platform_gated else True
    return False


def _resolve_one(
    name: str,
    entries: typ.Iterable[Override],
    reader: FilterReader,
    baseline: EffectiveTier,
) -> EffectiveTier:
    """Return the tier nextest gives ``name``, walking the entries in order.

    The first entry that both selects the name and configures ``slow-timeout``
    supplies it, which is nextest's per-setting rule: an entry setting only
    ``test-group`` does not shadow an earlier or later allowance. ``baseline``
    is the tier in force when no override supplies one, and carries both the
    figure and the profile path a name falls back to.

    Whether an entry supplies a figure is :func:`_decide`'s three-way answer,
    and the walk treats each outcome as one case: *yes* returns the figure,
    *no* leaves the tier alone, and *undecidable* supplies nothing but marks
    the name. The mark is what keeps the two unreadable readings
    distinguishable from a decided ``no``: a name no gated entry can select
    resolves cleanly and unmarked, while one a gate might claim is refused by
    the caller rather than reported at a figure nextest may not give it.

    An earlier shadowing mark survives a later match: ``unresolved`` records
    that *some* entry above the one supplying this figure might have claimed
    the name first, which a match further down does not dispel.

    Examples
    --------
    An entry selecting nothing leaves the baseline in place:

    >>> base = EffectiveTier(600.0, 'profile.default', False, False)
    >>> _resolve_one("b", [
    ...     Override("profile.default.overrides[0]", "binary(a)",
    ...              {"slow-timeout": {"period": "120s", "terminate-after": 8}}),
    ... ], selected_by_universe(["a", "b"]), base)
    EffectiveTier(allowance=600.0, path='profile.default', is_above_base=False, is_unresolved=False)

    A platform-gated entry supplies nothing and the raised entry below it
    supplies the figure. Whether the name is marked depends on one thing: a
    gate narrows its filter rather than replacing it, so a readable filter
    that excludes the name settles the question, and one that includes it
    leaves the platform undecided:

    >>> raised = Override(
    ...     "profile.default.overrides[1]", "binary(a)",
    ...     {"slow-timeout": {"period": "120s", "terminate-after": 8}},
    ... )
    >>> def gated(expression):
    ...     return Override(
    ...         "profile.default.overrides[0]", expression,
    ...         {"platform": "cfg(unix)",
    ...          "slow-timeout": {"period": "60s", "terminate-after": 10}},
    ...     )
    >>> _resolve_one("a", [gated("binary(b)"), raised], selected_by_universe(["a"]), base)
    EffectiveTier(allowance=960.0, path='profile.default.overrides[1]', is_above_base=True, is_unresolved=False)
    >>> _resolve_one("a", [gated("binary(a)"), raised], selected_by_universe(["a"]), base)
    EffectiveTier(allowance=960.0, path='profile.default.overrides[1]', is_above_base=True, is_unresolved=True)
    """
    tier = baseline
    for override in entries:
        if "slow-timeout" not in override.entry:
            continue
        supplies = _decide(override, name, reader)
        if supplies is None:
            tier = tier._replace(is_unresolved=True)
        elif supplies:
            return _tier_from(override, baseline.allowance)._replace(
                is_unresolved=tier.is_unresolved
            )
    return tier


def effective_allowances(
    config_text: str,
    names: typ.Iterable[str],
    *,
    reader: FilterReader,
    profile: str = "default",
) -> dict[str, EffectiveTier]:
    """Return the tier nextest gives each name, resolving shadowing.

    The entries are walked in file order and the first that selects a name
    *and* configures ``slow-timeout`` supplies it, which is nextest's rule.
    An entry that selects a name but configures another setting does not
    shadow it, because precedence is resolved per setting. An entry this
    reader cannot attribute a figure to -- gated, expression-less, or one it
    refused -- supplies nothing, and marks a name it might select unresolved
    rather than being skipped.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.
    names : iterable of str
        The binary names to resolve.
    reader : FilterReader
        Reads a filter expression into each candidate's selection. Passed in
        because the universe is the caller's -- the class and its complement
        -- and this reader must not invent one.
    profile : str
        The profile to resolve under.

    Returns
    -------
    dict of str to EffectiveTier
        One entry per name, in the order given.

    Examples
    --------
    ``a``'s first entry carries the base figure, so it wins and ``a`` is not
    raised despite the raised entry below. ``b``'s first entry sets only
    ``test-group``, which does not shadow that setting, so ``b`` takes the
    raised entry:

    >>> config = (
    ...     '[profile.default]\\nslow-timeout = { period = "60s", '
    ...     'terminate-after = 10 }\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(a)"\\n'
    ...     'slow-timeout = { period = "60s", terminate-after = 10 }\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(a)"\\n'
    ...     'slow-timeout = { period = "120s", terminate-after = 8 }\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(b)"\\n'
    ...     'test-group = "shared"\\n'
    ...     '[[profile.default.overrides]]\\nfilter = "binary(b)"\\n'
    ...     'slow-timeout = { period = "120s", terminate-after = 8 }\\n'
    ... )
    >>> resolved = effective_allowances(
    ...     config, ["a", "b"], reader=selected_by_universe(["a", "b"])
    ... )
    >>> [(n, resolved[n].path, resolved[n].is_above_base) for n in ("a", "b")]
    [('a', 'profile.default.overrides[0]', False), ('b', 'profile.default.overrides[3]', True)]
    """
    base = profile_allowance(config_text, profile)
    baseline = EffectiveTier(
        allowance=base,
        path=f"profile.{profile}",
        is_above_base=False,
        is_unresolved=False,
    )
    entries = overrides_in_order(config_text, profile)
    return {name: _resolve_one(name, entries, reader, baseline) for name in names}


def selected_by_universe(candidates: typ.Iterable[str]) -> FilterReader:
    """Return a :class:`FilterReader` over a fixed set of candidates.

    A convenience for callers, and the one this module's own tests use, so
    the reader passed to :func:`effective_allowances` is the same reader the
    contracts use rather than a stub that always agrees with them.

    Parameters
    ----------
    candidates : iterable of str
        The binary names to resolve expressions against.

    Returns
    -------
    FilterReader
        A reader from a filter expression to each candidate's selection,
        carrying :data:`REFUSES` as the exception it raises for a shape it
        cannot evaluate.

    Examples
    --------
    >>> reader = selected_by_universe(["a", "b"])
    >>> reader.read('binary(a)')
    {'a': True, 'b': False}
    >>> reader.refuses is REFUSES
    True
    """
    universe = list(candidates)

    def read(expression: str) -> dict[str, bool]:
        return binaries_selected_by(expression, universe)

    return FilterReader(read=read, refuses=REFUSES)


#: Re-exported so a caller can name the exception it passes as ``refuses``
#: without importing the filter language module as well.
REFUSES: typ.Final[type[Exception]] = UnreadableMatcherError
