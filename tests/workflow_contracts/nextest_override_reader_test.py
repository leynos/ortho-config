"""What one profile's override entries are read as, and in what order.

``overrides_in_order`` is the single place a profile's entries are extracted,
and every allowance question downstream inherits its order and its treatment
of a filter it cannot use as a filterset. The repository's own configuration
exercises the ordinary shape -- one entry, one string filter -- so these tests
drive the reader with documents it does not contain.

Each states the wrong answer its mistake would have produced. Order is the
sharp one: nextest applies entries in declaration order, so a reader that
returned them sorted, reversed, or in another profile's sequence would make a
later entry appear to shadow an earlier one and report a binary's allowance
against an entry the run never consults.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from nextest_override_reader import overrides_in_order

BASE: str = '[profile.default]\nslow-timeout = "60s"\n'
"""A profile with no overrides of its own, as a base for the documents below."""


def test_entries_are_returned_in_declaration_order() -> None:
    """The list is nextest's own order, not a sorted or reversed one.

    The three filters are distinct and their file order is not alphabetical,
    so sorting by expression or path, or reversing the list, changes the result
    rather than happening to reproduce it.
    """
    config = (
        BASE
        + '[[profile.default.overrides]]\nfilter = "binary(third)"\n'
        + '[[profile.default.overrides]]\nfilter = "binary(first)"\n'
        + '[[profile.default.overrides]]\nfilter = "binary(second)"\n'
    )
    entries = overrides_in_order(config)
    assert [entry.expression for entry in entries] == [
        "binary(third)",
        "binary(first)",
        "binary(second)",
    ], f"the entries left declaration order: {entries!r}"
    assert [entry.path for entry in entries] == [
        "profile.default.overrides[0]",
        "profile.default.overrides[1]",
        "profile.default.overrides[2]",
    ], f"the entry paths do not name the declarations they came from: {entries!r}"


def test_an_entry_without_a_filter_selects_without_a_filterset() -> None:
    """A platform-gated entry carries no filter, and is reported as gated.

    Reading the absence of a filter as "matches every name" would apply a
    Windows-only allowance to a Linux run; reading the gate as absent would
    lose the reason the entry has no filter at all.
    """
    config = BASE + '[[profile.default.overrides]]\nplatform = "cfg(windows)"\n'
    (entry,) = overrides_in_order(config)
    assert entry.expression is None, (
        f"an entry with no filter was given the expression {entry.expression!r}"
    )
    assert entry.is_platform_gated is True, (
        "the entry's platform gate was not reported, so a caller would apply "
        "it to every platform"
    )


def test_a_non_string_filter_is_not_read_as_a_filterset() -> None:
    """A ``filter`` that is not a string supplies no expression, and is kept.

    A reader that passed the parsed value through would hand a list or a number
    to the filterset reader, which might raise where the configuration is only
    odd, or stringify it into a pattern that matches nothing and report the
    entry as never applying. The entry's other settings still apply, so it has
    to stay in the list with its gate and its budget visible.
    """
    config = (
        BASE
        + "[[profile.default.overrides]]\nfilter = 7\n"
        + 'slow-timeout = "90s"\n'
    )
    (entry,) = overrides_in_order(config)
    assert entry.expression is None, (
        f"a non-string filter was read as the expression {entry.expression!r}"
    )
    assert entry.entry["slow-timeout"] == "90s", (
        "the entry was dropped rather than kept with its filter unreadable"
    )


def test_another_profiles_entries_are_not_read() -> None:
    """Only the named profile's list is returned.

    Reading several profiles together is how coverage comes to be claimed by an
    entry the run never consults, so a document listing another profile's
    overrides must contribute nothing.
    """
    config = (
        BASE
        + '[[profile.ci.overrides]]\nfilter = "binary(a)"\n'
        + '[[profile.default.overrides]]\nfilter = "binary(b)"\n'
    )
    entries = overrides_in_order(config)
    assert [entry.expression for entry in entries] == ["binary(b)"], (
        f"an entry from another profile was read as the default's: {entries!r}"
    )