"""What the allowance resolver does with entries it cannot fully evaluate.

The contract in ``child_cargo_tier_test`` asks one question of the resolver:
does this binary's effective allowance exceed the profile's own? The resolver
answers it correctly for the shapes this repository writes today, so the
repository alone cannot show that it *would* answer correctly for a shape one
key away -- an entry carrying both a ``filter`` and a ``platform`` gate, or a
filter language the reader refuses.

Each test below drives the resolver with a controlled configuration and states
the wrong answer the mistake would have produced. The distinction they turn on
is directional: an entry that is wrongly *applied* reports a binary as raised
when nextest gives it the earlier figure, which is the failure the whole
contract exists to catch, so every ambiguous shape is read as not supplying.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest
from nextest_precedence import (
    EffectiveTier,
    effective_allowances,
    selected_by_universe,
)

BASE_60: str = '[profile.default]\nslow-timeout = { period = "60s", terminate-after = 10 }\n'
"""A profile whose own allowance is 600 s, the repository's base tier."""

RAISED_120: str = (
    "[[profile.default.overrides]]\n"
    'filter = "binary(a)"\n'
    'slow-timeout = { period = "120s", terminate-after = 8 }\n'
)
"""An entry nextest resolves to 960 s for ``a``."""


def _resolve(config: str, *names: str) -> dict[str, EffectiveTier]:
    """Resolve ``names`` under ``config`` against exactly those candidates."""
    return effective_allowances(
        config, list(names), reader=selected_by_universe(list(names))
    )


def test_a_gated_entry_alongside_a_filter_does_not_supply_an_allowance() -> None:
    """A ``platform`` key is not detectable by a missing ``filter``.

    The gate and the filter are independent keys: nextest accepts an entry
    carrying both, and the gate then narrows the filter rather than replacing
    it. The mistake this guards is reading such an entry's filter and ignoring
    its gate, which applies an entry to a lane where it may not run at all.
    Measured on the real configuration, the trybuild entry that reserves every
    nextest slot on Windows has exactly this shape, so it is not a
    hypothetical.

    Wrong answer: ``a`` resolves to ``overrides[0]`` at the base figure with
    ``is_above_base`` false, because the gated entry is read as a plain base
    entry that matches. That is the reported-as-shadowed direction, and it
    would fail the coverage assertion for a configuration nextest runs
    correctly.
    """
    gated = (
        "[[profile.default.overrides]]\n"
        'filter = "binary(a)"\n'
        "platform = 'cfg(unix)'\n"
        'slow-timeout = { period = "60s", terminate-after = 10 }\n'
    )
    resolved = _resolve(BASE_60 + gated + RAISED_120, "a")["a"]
    assert resolved.path == "profile.default.overrides[1]", (
        f"the platform-gated entry at overrides[0] supplied {resolved.path}; a "
        f"gate this reader cannot evaluate must not be applied, or an entry "
        f"reserved for another platform decides this lane's reading"
    )
    assert resolved.is_above_base, (
        "the raised entry below the gate is the one nextest applies on Linux, "
        "so the binary must resolve as raised"
    )


def test_a_gated_entry_that_might_claim_the_name_leaves_it_unresolved() -> None:
    """A gate is not evidence the entry does not apply; only its filter is.

    The gate is refused because the platform is unknown, not because the entry
    cannot match. The same gate is true on one lane and false on another, so an
    entry carrying ``cfg(unix)`` above a raised entry shadows it on the very
    lane these contracts assert, and reporting the raised figure as certain is
    the fail-open direction: coverage is claimed for a configuration whose
    member nextest may run at the earlier figure.

    Wrong answer: ``is_unresolved`` false, from skipping the gated entry
    instead of marking it.
    """
    gated = (
        "[[profile.default.overrides]]\n"
        'filter = "binary(a)"\n'
        "platform = 'cfg(unix)'\n"
        'slow-timeout = { period = "60s", terminate-after = 10 }\n'
    )
    resolved = _resolve(BASE_60 + gated + RAISED_120, "a")["a"]
    assert resolved.is_unresolved, (
        "a gate naming this binary may apply on the lane being read, so the "
        "figure below it is not known to be the one nextest applies"
    )


def test_a_gated_entry_whose_filter_excludes_the_name_does_not_mark_it() -> None:
    """The readable half of a gated entry still settles the question.

    A gate *narrows* its filter rather than replacing it, so a name the filter
    excludes is excluded on every platform and no decision is pending for it.
    This is what keeps the mark usable: the real configuration's Windows-only
    trybuild entry names four binaries this class does not contain, and marking
    every member for it would make the contract's unresolved assertion fire on
    a configuration that is correct.

    Wrong answer: ``is_unresolved`` true, from treating every gated entry as
    possibly naming anything.
    """
    gated = (
        "[[profile.default.overrides]]\n"
        'filter = "binary(other_binary)"\n'
        "platform = 'cfg(windows)'\n"
        'slow-timeout = { period = "60s", terminate-after = 10 }\n'
    )
    resolved = _resolve(BASE_60 + gated + RAISED_120, "a")["a"]
    assert not resolved.is_unresolved, (
        "the gated entry's readable filter excludes this name, so the platform "
        "decision cannot change whether it claims it"
    )
    assert resolved.path == "profile.default.overrides[1]"
    assert resolved.is_above_base


def test_an_ungated_platform_entry_marks_every_name() -> None:
    """An expression-less entry is marked the same way as a gated one.

    An entry selecting by ``platform`` alone carries no ``filter``, so it names
    no binary this reader can evaluate and no name can be ruled out. Treating
    it as supplying nothing is the same decision as the gated case, reached
    through the other of the two shapes an unevaluable entry takes; the pair is
    what shows the rule is about what can be decided rather than about the
    ``filter`` key's absence.
    """
    bare = (
        "[[profile.default.overrides]]\n"
        "platform = 'cfg(windows)'\n"
        'slow-timeout = { period = "60s", terminate-after = 10 }\n'
    )
    resolved = _resolve(BASE_60 + bare + RAISED_120, "a")["a"]
    assert resolved.path == "profile.default.overrides[1]"
    assert resolved.is_above_base
    assert resolved.is_unresolved


def test_a_refused_expression_shadows_and_does_not_supply() -> None:
    """An unreadable entry above a match leaves the name unresolved.

    The shadowing mark must survive the later match. A mark that a match
    further down dispels would report a figure as certain while an entry above
    it might have claimed the name first, which is precisely the case a caller
    asserting a tier must refuse.

    Wrong answer: ``is_unresolved`` false, from clearing the mark on match.
    """
    unreadable = (
        "[[profile.default.overrides]]\n"
        'filter = "binary(a) & test(slow)"\n'
        'slow-timeout = { period = "60s", terminate-after = 10 }\n'
    )
    resolved = _resolve(BASE_60 + unreadable + RAISED_120, "a")["a"]
    assert resolved.is_unresolved, (
        "an entry this reader cannot evaluate sits above the entry that "
        "supplied the figure, and it names this binary, so the allowance is "
        "not known to be the one nextest applies"
    )


@pytest.mark.parametrize("name", ["a", "unrelated_binary"])
def test_only_the_entry_that_matches_supplies_the_figure(name: str) -> None:
    """The resolver returns the first matching entry's figure, not the last.

    The positive control for the three tests above: with nothing unevaluable in
    the file, the walk must reach the raised entry and return its figure. A
    resolver that skipped every entry, or that let a non-matching one supply,
    would fail here rather than passing the negative cases for the wrong
    reason.
    """
    resolved = _resolve(BASE_60 + RAISED_120, name)[name]
    if name == "a":
        assert resolved.path == "profile.default.overrides[0]"
        assert resolved.is_above_base
    else:
        assert resolved.path == "profile.default"
        assert not resolved.is_above_base
    assert not resolved.is_unresolved
