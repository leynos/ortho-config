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
    CHILD_CARGO_TIER_CRATES,
    UnreadableMatcherError,
    binaries_selected_by,
    child_cargo_binaries,
    marker_binaries,
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
    ), "the unshadowed configuration must raise the whole class, or the "
    "fixture's change is not the one being demonstrated"
    tiers = _effective_tiers(shadowed)
    on_base = sorted(name for name, tier in tiers.items() if not tier.is_above_base)
    assert on_base == ["cli_dispatch"], (
        f"resolving the shadowed configuration reported {on_base} on the base "
        f"allowance; the entry inserted above the raised one names "
        f"cli_dispatch at the base figure, so it is the only member whose "
        f"allowance that entry takes away"
    )


def _fixture_workspace(tmp_path: pathlib.Path, binary: str) -> pathlib.Path:
    """Write a minimal workspace whose one test binary drives a child cargo.

    Two crates are written because the reader walks a fixed list of them; the
    first carries the binary and the second is empty. The binary reaches its
    spawn through a helper in a submodule, so the fixture exercises the module
    graph the markers are read over rather than only a flat file.

    The helper sits in ``support/`` rather than beside the binary, which is
    both the layout the real repository uses for ``tests/fixtures`` and the one
    cargo agrees with: a ``.rs`` file directly in ``tests/`` is compiled as its
    own test binary, which is why shared helpers are put in a subdirectory. A
    fixture with ``helper.rs`` beside the binary would be read as two binaries,
    correctly.

    Parameters
    ----------
    tmp_path : pathlib.Path
        The test's temporary directory.
    binary : str
        The stem to give the test binary.

    Returns
    -------
    pathlib.Path
        The workspace root, suitable as the reader's ``base``.
    """
    for crate in CHILD_CARGO_TIER_CRATES:
        (tmp_path / crate / "tests" / "support").mkdir(parents=True)
        (tmp_path / crate / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
    roots = tmp_path / CHILD_CARGO_TIER_CRATES[0] / "tests"
    (roots / f"{binary}.rs").write_text("mod support;\n", encoding="utf-8")
    (roots / "support" / "mod.rs").write_text(
        'fn run() {\n    let _ = Command::new("cargo");\n}\n', encoding="utf-8"
    )
    return tmp_path


def test_the_class_is_read_from_the_sources_not_from_the_filter(
    tmp_path: pathlib.Path,
) -> None:
    """The class follows the Rust, not the configuration naming it.

    The member list is a fact about the sources, and this asserts that by
    changing the sources while holding the configuration fixed. A reader that
    derived the class from the override's filter -- the reading that would make
    every other assertion in this module vacuous, since it would agree with the
    filter by construction -- reports the certified class for the second
    workspace below and fails here.

    The first part states the fact the other tests rest on for the real
    repository; the second is the controlled input that separates the two
    readings.
    """
    class_members = child_cargo_binaries(REPOSITORY_ROOT)
    assert class_members, "the child-cargo class must not be empty"
    non_members = non_child_cargo_binaries(REPOSITORY_ROOT)
    assert class_members & non_members == set(), (
        "a binary cannot both drive a child cargo and not drive one; the two "
        "readings disagree about which binaries exist"
    )
    # The fixture's source names a binary the configuration has never heard of.
    # The configuration is not consulted here at all, so a class that follows
    # the filter rather than the sources cannot produce this name.
    fixture = _fixture_workspace(tmp_path, "a_binary_no_override_names")
    assert child_cargo_binaries(fixture) == {"a_binary_no_override_names"}, (
        "the reader did not follow the fixture's sources; the class must come "
        "from what the Rust compiles, and the certified configuration names no "
        "such binary"
    )
    assert child_cargo_binaries(fixture) != class_members, (
        "the fixture reproduces the certified class, so it cannot show the "
        "reading follows the sources rather than the configuration"
    )


def test_a_source_backed_member_with_no_override_is_reported(
    tmp_path: pathlib.Path,
) -> None:
    """A member added to the sources but not the configuration fails the tier.

    The second controlled input: the configuration is held at its certified
    text and the class is changed, which is the omission issue #611 recorded
    for ``cli_dispatch`` and the direction a hand-maintained entry list drifts
    in. The assertion is the same one the coverage test makes, so this shows
    that assertion failing for the reason it exists rather than only passing.
    """
    certified = NEXTEST_CONFIG.read_text(encoding="utf-8")
    fixture = _fixture_workspace(tmp_path, "a_binary_no_override_names")
    universe = sorted(child_cargo_binaries(fixture) | {"cli_dispatch"})
    tiers = effective_allowances(
        certified,
        sorted(child_cargo_binaries(fixture)),
        reader=selected_by_universe(universe),
    )
    on_base = sorted(name for name, tier in tiers.items() if not tier.is_above_base)
    assert on_base == ["a_binary_no_override_names"], (
        f"a class member the configuration does not name resolved to {on_base}; "
        f"the fixture's binary has no override, so it must be the only member "
        f"on the profile's own allowance, exactly as cli_dispatch was"
    )
    assert not any(tier.is_unresolved for tier in tiers.values()), (
        "no entry in the certified configuration is unreadable, so a member "
        "must not be reported as unresolved"
    )
