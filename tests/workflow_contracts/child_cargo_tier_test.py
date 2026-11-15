"""Contract for the child-cargo allowance and the test-group gap.

Two defects, one contract. The first is that a binary whose tests spawn
``cargo`` may sit on the base allowance: its wall clock contains a nested
build, and the base tier is sized for tests that do not run one. The second is
that a test-group override may carry no ``slow-timeout`` at all, so the
binaries serialized *because* they contend for a shared build directory fall
through to the base allowance by omission rather than by decision. Issue #611
recorded both: ``rstest_bdd``, ``powershell_windows`` and
``nested_subcommand_end_to_end`` each had a group and no allowance between
them, and ``cli_dispatch`` was absent from the list entirely.

The class is read from the Rust sources by :mod:`child_cargo_tier`. What this
module asserts is that the configuration's filters select *exactly* that class,
in both directions, so neither an omission nor an over-broad filter can pass.

The overrides are found by **membership**, not by allowance. Three overrides in
this file carry 960s, so keying on the figure is ambiguous and would refuse to
read at all. What the contract asserts instead is the allowance nextest
*resolves* for each member: the entries are walked in file order, and a member
takes the first one that both selects it and supplies a ``slow-timeout``.
Naming a member in a raised entry is therefore not the same as raising it -- an
entry above it that matches and carries the base figure wins, and a union of the
entries that merely mention the member cannot see that. The class's members may
also be spread across more than one above-base entry; they are here, because
``rstest_bdd`` has its own measured 956-second scenario envelope. Per-member
resolution handles both without a union.

Two further assertions protect the shared serialization. Every audited binary
a test group serializes must be a class member, because the raised allowance is
written for the class and a serialized non-member would fall through to the
base tier. Groups selecting only binaries outside this inventory belong to
their own contracts. Read from the raw tables rather than from
:func:`nextest_budgets.override_allowances`, which skips an entry declaring no
``slow-timeout`` -- correctly, since it grants no budget to compare -- and so
cannot see that omission at all.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import typing as typ

import pytest
from child_cargo_tier import (
    UnreadableMatcherError,
    binaries_selected_by,
    child_cargo_binaries,
    non_child_cargo_binaries,
)
from nextest_budgets import override_allowances, profile_allowance
from nextest_document import budget_tables
from nextest_precedence import (
    EffectiveTier,
    effective_allowances,
    selected_by_universe,
)

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
NEXTEST_CONFIG: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".config" / "nextest.toml"


@pytest.fixture(scope="module")
def nextest_config() -> str:
    """Return the nextest configuration's text, read once."""
    return NEXTEST_CONFIG.read_text(encoding="utf-8")


def _class_covering_overrides(
    config_text: str,
) -> list[tuple[str, str, float, set[str]]]:
    """Return each above-base override that selects at least one class member.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    list of (str, str, float, set of str)
        Each entry's path, filter expression, allowance, and the binaries it
        selects. An entry whose selection misses the class entirely is left
        out: this repository has a second raised class, the trybuild binaries,
        and an entry that raises those is not claiming anything about this one.

    Raises
    ------
    UnreadableMatcherError
        If a filter is not a ``binary`` disjunction this contract can read.
    """
    base = profile_allowance(config_text)
    class_members = child_cargo_binaries(REPOSITORY_ROOT)
    universe = sorted(class_members | non_child_cargo_binaries(REPOSITORY_ROOT))
    covering = []
    for path, expression, budget in override_allowances(config_text):
        if expression is None or budget <= base:
            continue
        selected = binaries_selected_by(expression, universe)
        chosen = {name for name, covered in selected.items() if covered}
        if chosen & class_members:
            covering.append((path, expression, budget, chosen))
    return covering


def _effective_tiers(config_text: str) -> dict[str, EffectiveTier]:
    """Return the allowance nextest gives each class member, resolving order.

    The entries are read from the configuration and each member's tier is
    resolved by file order, so an entry that mentions a member without
    supplying its allowance does not count as covering it.

    Parameters
    ----------
    config_text : str
        The nextest configuration file's text.

    Returns
    -------
    dict of str to EffectiveTier
        One entry per class member.
    """
    class_members = child_cargo_binaries(REPOSITORY_ROOT)
    universe = sorted(class_members | non_child_cargo_binaries(REPOSITORY_ROOT))
    return effective_allowances(
        config_text,
        sorted(class_members),
        reader=selected_by_universe(universe),
    )


def test_every_child_cargo_binary_is_covered_by_an_override(
    nextest_config: str,
) -> None:
    """The class read from the Rust sources is exactly what the filters select.

    Two directions, and they are read differently because they are different
    questions. Whether a member is *raised* is a question about the allowance
    nextest resolves for it, so it is answered by walking the entries in file
    order; a member listed in a raised entry that an earlier entry shadows is
    not raised. ``cli_dispatch`` ran on the base allowance with a nested build
    on its clock until this contract existed, and nextest's own precedence
    rule is what a later entry list would otherwise hide.

    Whether an entry's filter is *too wide* is a question about that entry's
    selection, which its own expression answers: an above-base entry naming a
    non-member grants a raised allowance the class does not need and hides a
    genuine member's omission, since the reading is against the class and not
    against the filter.
    """
    class_members = child_cargo_binaries(REPOSITORY_ROOT)
    # A contract over an empty class would pass for every filter, including
    # one naming nothing, so the class is required to be a real set.
    assert len(class_members) > 1, (
        f"the child-cargo class read as {sorted(class_members)}; a class this "
        f"small makes the coverage assertion below vacuous"
    )
    covering = _class_covering_overrides(nextest_config)
    assert covering, (
        "no above-base override selects any child-cargo binary, so the whole "
        "class is on the base allowance; this contract reads the class's "
        "allowances from those entries and an empty set proves nothing"
    )
    for path, expression, _, chosen in covering:
        extra = sorted(chosen - class_members)
        assert not extra, (
            f"{path} selects {extra}, which drive no child cargo, so they take "
            f"a raised allowance they do not need: {expression!r}. A filter "
            f"wider than the class also hides a genuine member's omission, "
            f"since the coverage assertion is against the class, not against "
            f"the filter's own reading"
        )
    tiers = _effective_tiers(nextest_config)
    unresolved = sorted(name for name, tier in tiers.items() if tier.is_unresolved)
    assert not unresolved, (
        f"{unresolved} have an override above their raised entry whose filter "
        f"this contract cannot read, so the allowance attributed to them is "
        f"not known to be the one nextest applies. Every entry in this file is "
        f"a readable binary disjunction, so this means a new shape arrived and "
        f"the reader must be extended before the tier can be asserted"
    )
    missing = sorted(name for name, tier in tiers.items() if not tier.is_above_base)
    assert not missing, (
        f"{missing} are child-cargo binaries but nextest resolves each to "
        f"{tiers[missing[0]].allowance:.0f}s, the profile's own allowance, so "
        f"each pays a nested build inside a test clock sized for tests that "
        f"start no child cargo. They are either named by no override, or named "
        f"by one that an earlier entry shadows; both are read the same way "
        f"here, by walking the entries in the order nextest applies them"
    )


def test_the_allowances_that_cover_the_class_are_above_the_base_one(
    nextest_config: str,
) -> None:
    """The class is raised, not merely moved onto a different number.

    The entries are read first, so the assertion is not satisfied by an
    override that happens to carry a raised figure while selecting nothing:
    the lookup would leave it out and the coverage test would report the whole
    class as unraised.
    """
    covering = _class_covering_overrides(nextest_config)
    base = profile_allowance(nextest_config)
    assert covering, "no above-base override selects any child-cargo binary"
    for path, _, allowance, _ in covering:
        assert base < allowance, (
            f"{path} covers child-cargo binaries at {allowance:.0f}s against a "
            f"base of {base:.0f}s; the class is raised because it is slower "
            f"than the base tier covers"
        )


def test_every_test_group_binary_is_covered_by_a_raised_tier(
    nextest_config: str,
) -> None:
    """No serialized binary falls through to the base allowance by omission.

    Serializing a binary through a test group is what makes sharing a build
    directory safe. It does not make the binary cheaper, and an entry that
    declares a group and no ``slow-timeout`` grants no budget: nextest resolves
    precedence separately for each setting, so the omission is invisible to
    anything reading allowances alone.

    Within this contract's audited binary universe, every binary a group
    serializes must be a class member, which is the class the raised entries
    cover. Groups that select only binaries outside that universe belong to
    other contracts. Read from the raw tables rather than from
    :func:`nextest_budgets.override_allowances`, which skips an entry declaring
    no ``slow-timeout`` and so cannot see the shape this test exists to catch.

    This test alone therefore proves membership, not a raised allowance. The
    conclusion comes from chaining it with
    :func:`test_every_child_cargo_binary_is_covered_by_an_override`, which
    asserts the other direction: serialized names are a subset of the class,
    and the class is a subset of the binaries resolved to an above-base tier.
    Neither half is redundant -- a group entry that declares ``test-group``
    and no ``slow-timeout`` still serializes, so the omission this test catches
    is invisible to anything reading allowances alone.
    """
    grouped = [
        (path, entry)
        for path, entry in budget_tables(nextest_config)
        if "overrides[" in path and "test-group" in entry
    ]
    assert grouped, (
        "no override assigns a test group; this contract reads the serialized "
        "binaries from those entries, and an empty set makes it vacuous"
    )
    class_members = child_cargo_binaries(REPOSITORY_ROOT)
    universe = sorted(class_members | non_child_cargo_binaries(REPOSITORY_ROOT))
    serialized: dict[str, str] = {}
    for path, entry in grouped:
        expression = entry.get("filter")
        assert isinstance(expression, str), (
            f"{path} assigns a test group but declares no filter, so it "
            f"serializes every test in the run; this contract cannot read a "
            f"platform gate as a filterset"
        )
        selected = binaries_selected_by(expression, universe)
        chosen = [name for name, covered in selected.items() if covered]
        for name in chosen:
            serialized[name] = path
    assert serialized, (
        "no test-group filter selects a binary this contract audits, so the "
        "group coverage assertion would be vacuous"
    )
    off_tier = sorted(
        f"{name} (assigned by {path})"
        for name, path in serialized.items()
        if name not in class_members
    )
    assert not off_tier, (
        f"{off_tier} are serialized into a test group but drive no child "
        f"cargo, so no raised allowance is written for them and they fall "
        f"through to the base 600s tier"
    )


def test_a_matcher_this_contract_cannot_read_is_refused() -> None:
    """An unreadable filter fails the contract rather than passing it.

    A conjunction rated by its ``binary`` terms alone would report a narrower
    expression as a wider one, so it is refused rather than misread.
    """
    with pytest.raises(
        UnreadableMatcherError, match="combines its supported terms"
    ):
        binaries_selected_by("binary(a) & test(b)", ["a"])




def test_a_base_tier_entry_above_a_raised_one_removes_coverage() -> None:
    """An entry that shadows a raised one is reported, not skipped.

    The negative control for the resolution above, driven by configuration
    alone. This is the shape the union of above-base entries could not see: the
    raised entry still names ``cli_dispatch``, so that reading reports the class
    as covered while nextest gives the binary the earlier entry's allowance --
    precedence is file order, first match per setting.

    Nothing about the class changes here, so a coverage reading that agrees with
    the filter in both directions still passes on the shadowed text. Only
    resolving the effective allowance fails it.
    """
    certified = NEXTEST_CONFIG.read_text(encoding="utf-8")
    shadow = (
        "[[profile.default.overrides]]\n"
        "filter = 'binary(cli_dispatch)'\n"
        'slow-timeout = { period = "60s", terminate-after = 10 }\n\n'
    )
    shadowed = certified.replace("[test-groups]", shadow + "[test-groups]", 1)
    assert shadowed != certified, "the fixture did not insert the entry"
    # The controlled input: the class is fixed for both readings, and only the
    # configuration differs. Comparing the two configurations' verdicts on the
    # same class is what separates a reader of the sources from one that echoes
    # the filter back.
    class_members = child_cargo_binaries(REPOSITORY_ROOT)
    assert class_members, "the class must be non-empty for this control"
    # The union this contract used to assert over still sees full coverage, so
    # the insertion is invisible to it and the assertion below is not redundant.
    covering = _class_covering_overrides(shadowed)
    raised: set[str] = set()
    for _, _, _, chosen in covering:
        raised |= chosen
    assert not (class_members - raised), (
        "the fixture is not a negative control: the union of above-base entries "
        "already reports the class as covered, so no assertion could fail here"
    )
    assert all(
        tier.is_above_base for tier in _effective_tiers(certified).values()
    ), (
        "the unshadowed configuration must raise the whole class, or the "
        "fixture's change is not the one being demonstrated"
    )
    tiers = _effective_tiers(shadowed)
    on_base = sorted(name for name, tier in tiers.items() if not tier.is_above_base)
    assert on_base == ["cli_dispatch"], (
        f"resolving the shadowed configuration reported {on_base} on the base "
        f"allowance; the entry inserted above the raised one names "
        f"cli_dispatch at the base figure, so it is the only member whose "
        f"allowance that entry takes away"
    )
