"""Contract for the trybuild per-test allowance and the binaries it covers.

The trybuild binaries are budgeted separately because each spawns a child
`cargo` in its own target directory, paying a cold dependency build that no
other test here pays. Two failures motivated this contract:

- The filter named two binaries while seven existed. The other five ran on
  the base allowance, and `must_use_compile_tests` was killed at 600.224 s
  on run 36070786646 while still cold-compiling its first dependencies.
- Adding a binary to the override *looks* like it raises that binary's
  budget, and did not: the override was 120 s x 5 and the base was
  60 s x 10, so both allowed 600 s and only the warning cadence differed.

So the two properties checked here are that every trybuild binary is
covered, and that the covering allowance is actually larger than the base
one. Neither is visible from the configuration file alone.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest
from nextest_budgets import (
    largest_test_allowance,
    override_allowances,
    profile_allowance,
)
from timeout_budgets import NEXTEST_CONFIG
from trybuild_tier import (
    TRYBUILD_CALL,
    UnreadableMatcherError,
    binaries_selected_by,
    trybuild_binaries,
)


@pytest.fixture(scope="module")
def nextest_config() -> str:
    """Return the nextest configuration file's text.

    Returns
    -------
    str
        The file's contents.
    """
    return NEXTEST_CONFIG.read_text(encoding="utf-8")


def _trybuild_filter(config_text: str) -> str:
    """Return the ``filter`` of the override carrying the largest allowance.

    That entry is the trybuild one: its allowance is the largest in the
    file. A second entry claiming the same figure would make this
    ambiguous, so the ambiguity is refused rather than resolved by position,
    because reading the wrong filter would have this contract certify a
    coverage gap it was written to find.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    str
        That entry's ``filter`` expression.

    Raises
    ------
    AssertionError
        If no entry, or more than one, carries the largest allowance.
    """
    largest = largest_test_allowance(config_text)
    carrying = [
        (path, expression)
        for path, expression, budget in override_allowances(config_text)
        if budget == pytest.approx(largest)
    ]
    assert len(carrying) == 1, (
        f"expected exactly one overrides entry at the largest per-test allowance "
        f"({largest:.0f}s); found {[path for path, _ in carrying]}. The trybuild "
        f"filter is read from that entry, so an ambiguous one is not read at all"
    )
    return carrying[0][1]


def test_every_trybuild_binary_is_covered_by_the_override(nextest_config: str) -> None:
    """The filter must select the whole class, not a sample of it.

    The sources say which binaries are trybuild ones and the filter says
    which it selects. Comparing the two is the only way to see a binary
    that belongs to the class and is not covered, which is how five of
    seven came to run on an allowance sized for tests that do not spawn a
    child cargo.

    A filter naming every binary in the workspace would pass this and mean
    nothing, so the class is also required to be non-trivial.
    """
    class_members = trybuild_binaries()
    assert len(class_members) > 1, (
        f"found {sorted(class_members)!r} as the trybuild binaries, which is too "
        f"few to be the class this override exists for; either the sources moved "
        f"or the reading of them broke and would now pass vacuously"
    )
    expression = _trybuild_filter(nextest_config)
    selected = binaries_selected_by(expression, sorted(class_members))
    missing = sorted(name for name, covered in selected.items() if not covered)
    assert not missing, (
        f"{missing} are trybuild binaries, so each spawns a child cargo and pays "
        f"a cold dependency build, but the override's filter does not select "
        f"them: {expression!r}. They run on the base allowance instead, "
        f"which is sized for tests that do not"
    )


def test_the_trybuild_allowance_is_above_the_base_one() -> None:
    """The override has to buy more than a different warning cadence.

    The override is what a binary is added to, so if its allowance is not
    larger than the profile's own, adding one achieves nothing. This is
    not hypothetical: the override was 120 s x 5 and the base 60 s x 10,
    which are the same product, and a binary moved between them gained no
    time at all.
    """
    config_text = NEXTEST_CONFIG.read_text(encoding="utf-8")
    base = profile_allowance(config_text)
    override = largest_test_allowance(config_text)
    assert override > base, (
        f"the trybuild override allows {override:.0f}s and the default profile "
        f"allows {base:.0f}s; an override no larger than the profile does not "
        f"give the binaries it covers any more time than they had"
    )


def test_a_matcher_this_contract_cannot_read_is_refused() -> None:
    """A silent misreading would report the wrong answer as a pass.

    The matchers the nextest reference lists are all read, and a form added
    later is not. Refusing one is the safe direction; treating it as
    matching nothing would report the whole class as uncovered, and as
    matching everything would report a broken filter as sound.
    """
    with pytest.raises(UnreadableMatcherError):
        binaries_selected_by("binary({a,b})", ["a"])
    with pytest.raises(UnreadableMatcherError):
        binaries_selected_by("test(something)", ["a"])


def test_the_trybuild_call_this_contract_matches_is_the_one_the_sources_use() -> None:
    """The reading keys on a literal, so the literal is pinned.

    `trybuild_binaries` finds a binary by matching `TRYBUILD_CALL` in its
    source. If trybuild's API were spelled differently in a new file, that
    file would be counted as no trybuild binary at all and the coverage
    assertion above would not ask for it.
    """
    assert TRYBUILD_CALL == "trybuild::TestCases::new()"
