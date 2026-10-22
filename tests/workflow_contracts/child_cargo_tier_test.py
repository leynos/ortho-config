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
read at all. Coverage is what matters instead: the class's members may be
spread across more than one above-base entry -- they are here, because
``rstest_bdd`` has its own measured 956-second scenario envelope -- so the
contract reads the union of every above-base entry that selects a class member,
and requires that union to be the class.

Two further assertions protect the shared serialization. Every binary a test
group serializes must be a class member, because the raised allowance is
written for the class and a serialized non-member would fall through to the
base tier. Read from the raw tables rather than from
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
    marker_binaries,
    non_child_cargo_binaries,
)
from nextest_budgets import override_allowances, profile_allowance
from nextest_document import budget_tables

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


def test_every_child_cargo_binary_is_covered_by_an_override(
    nextest_config: str,
) -> None:
    """The class read from the Rust sources is exactly what the filters select.

    Both directions are asserted, over the union of the above-base entries that
    claim any part of the class. A member no entry raises runs on the base
    allowance with a nested build on its clock, which is the defect:
    ``cli_dispatch`` ran there until this contract existed. A non-member that
    such an entry still selects takes a raised allowance it does not need, and
    an over-wide filter also hides a genuine member's omission, since the
    equality asserted is against the class and not against the filter's own
    reading.
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
    raised: set[str] = set()
    for path, expression, _, chosen in covering:
        extra = sorted(chosen - class_members)
        assert not extra, (
            f"{path} selects {extra}, which drive no child cargo, so they take "
            f"a raised allowance they do not need: {expression!r}. A filter "
            f"wider than the class also hides a genuine member's omission, "
            f"since the coverage assertion is against the class, not against "
            f"the filter's own reading"
        )
        raised |= chosen
    missing = sorted(class_members - raised)
    assert not missing, (
        f"{missing} are child-cargo binaries, so each pays a nested build "
        f"inside its own test's wall clock, but no above-base override selects "
        f"them. They run on the base allowance, which is sized for tests that "
        f"start no child cargo"
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

    The property asserted is that every binary a group serializes is a class
    member, which is the class the raised entries cover. Read from the raw
    tables rather than from :func:`nextest_budgets.override_allowances`, which
    skips an entry declaring no ``slow-timeout`` and so cannot see the shape
    this test exists to catch.
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
        assert chosen, (
            f"{path} serializes nothing this contract audits: {expression!r}. "
            f"A group whose filter names no known binary makes the coverage "
            f"assertion below vacuous while the group still applies to "
            f"whatever it does match"
        )
        for name in chosen:
            serialized[name] = path
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
    with pytest.raises(UnreadableMatcherError, match="combines its binary"):
        binaries_selected_by("binary(a) & test(b)", ["a"])


def test_every_marker_still_selects_a_binary() -> None:
    """Each marker earns its place in the reading.

    A marker whose subject was renamed matches nothing and silently shrinks the
    class, which the coverage assertion above cannot report: it compares the
    class with the filter, and both agree once the member is gone. The
    ``cargo-runner`` marker did exactly that when the shared helper was renamed
    from ``prepare_cargo_command``, and the union still held ten members, so
    nothing else noticed.

    Liveness is asserted **workspace-wide**, not per crate, because the markers
    are not per-crate facts: ``cargo_orthohelp_exe`` is a
    ``cargo-orthohelp``-only export, so requiring it in ``ortho_config`` would
    demand a ``binary-loader`` hit that no honest source can supply. What each
    label must do is select at least one binary somewhere in the audited
    crates, since that is what keeps its members in the class.
    """
    per_marker = marker_binaries(REPOSITORY_ROOT)
    assert per_marker, "the reader must report a group per marker"
    empty = sorted(label for label, hits in per_marker.items() if not hits)
    assert not empty, (
        f"{empty} select no binary in any audited crate; either the helper "
        f"each names was renamed, or the repository stopped spawning cargo "
        f"that way. A marker that matches nothing removes its members from the "
        f"class, and the coverage assertion cannot see that because it compares "
        f"the class with a filter that agrees with it"
    )
    union: set[str] = set()
    for hits in per_marker.values():
        union |= hits
    assert union == set(child_cargo_binaries(REPOSITORY_ROOT)), (
        "the per-marker sets and the class read from the same files disagree; "
        "one of the two readings is not reporting what the module graph says"
    )


def test_the_class_is_read_from_the_sources_not_from_the_filter() -> None:
    """The member list is a fact about the Rust, so it is stable and non-empty.

    Stated as its own assertion because every other test in this module is
    downstream of the reading: if the reader were replaced by one that echoed
    the filter back, the others would all pass while proving nothing.
    """
    class_members = child_cargo_binaries(REPOSITORY_ROOT)
    assert class_members, "the child-cargo class must not be empty"
    non_members = non_child_cargo_binaries(REPOSITORY_ROOT)
    assert class_members & non_members == set(), (
        "a binary cannot both drive a child cargo and not drive one; the two "
        "readings disagree about which binaries exist"
    )
