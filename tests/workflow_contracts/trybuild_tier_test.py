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

import os
import pathlib

import pytest
from nextest_budgets import (
    largest_test_allowance,
    override_allowances,
    profile_allowance,
)
from timeout_budgets import NEXTEST_CONFIG
from trybuild_tier import (
    TRYBUILD_CALL,
    ScanError,
    UnreadableMatcherError,
    binaries_selected_by,
    non_trybuild_binaries,
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
    path, expression = carrying[0]
    assert expression is not None, (
        f"{path} carries the largest per-test allowance but declares no filter, "
        f"so it selects every test rather than the trybuild class; this contract "
        f"reads a filterset from that entry and cannot read a platform gate as one"
    )
    return expression


def test_every_trybuild_binary_is_covered_by_the_override(nextest_config: str) -> None:
    """The filter must select the whole class, not a sample of it.

    The sources say which binaries are trybuild ones and the filter says
    which it selects. Comparing the two is the only way to see a binary
    that belongs to the class and is not covered, which is how five of
    seven came to run on an allowance sized for tests that do not spawn a
    child cargo.

    A filter naming every binary in the workspace would pass a coverage
    check that offers it nothing but class members to match against, so
    every other test binary is offered alongside them and the two sets
    compared for equality. The class is also required to be non-trivial.
    """
    class_members = trybuild_binaries()
    assert len(class_members) > 1, (
        f"found {sorted(class_members)!r} as the trybuild binaries, which is too "
        f"few to be the class this override exists for; either the sources moved "
        f"or the reading of them broke and would now pass vacuously"
    )
    expression = _trybuild_filter(nextest_config)
    others = non_trybuild_binaries()
    selected = binaries_selected_by(expression, sorted(class_members | others))
    chosen = {name for name, covered in selected.items() if covered}
    missing = sorted(class_members - chosen)
    assert not missing, (
        f"{missing} are trybuild binaries, so each spawns a child cargo and pays "
        f"a cold dependency build, but the override's filter does not select "
        f"them: {expression!r}. They run on the base allowance instead, "
        f"which is sized for tests that do not"
    )
    extra = sorted(chosen - class_members)
    assert not extra, (
        f"the override's filter also selects {extra}, which carry no trybuild "
        f"coverage: {expression!r}. An over-wide filter spends the trybuild "
        f"allowance on tests that do not need it, and can hide a class member "
        f"being dropped from it — the coverage assertion above cannot see the "
        f"difference if every name it asks about is matched anyway"
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
    """The reading keys on a pattern, so the pattern is pinned.

    `trybuild_binaries` finds a binary by matching `TRYBUILD_CALL` in its
    source. If trybuild's API were spelled differently in a new file, that
    file would be counted as no trybuild binary at all and the coverage
    assertion above would not ask for it.
    """
    assert TRYBUILD_CALL.search("trybuild::TestCases::new()") is not None
    # Rust accepts whitespace before the argument list, and a source that
    # used it would otherwise be read as no trybuild binary at all.
    assert TRYBUILD_CALL.search("let t = trybuild::TestCases::new ();") is not None


def test_a_filter_combining_binary_terms_is_refused() -> None:
    """A narrower expression must not be read as a plain disjunction.

    `binaries_selected_by` reads only `binary(...)` terms. A conjunction,
    difference, or negation beside them changes which tests run — the
    example below selects nothing when `nonexistent` matches no test —
    so reading the terms alone would report the expression as wider than
    it is.
    """
    class_members = trybuild_binaries()
    assert class_members, "this case needs a class to test against"
    member = sorted(class_members)[0]
    for compound in (
        f"binary({member}) & test(nonexistent)",
        f"binary({member}) - binary(other)",
        f"not(binary({member}))",
        f"binary({member}) & !test(a)",
    ):
        with pytest.raises(UnreadableMatcherError):
            binaries_selected_by(compound, sorted(class_members))


def test_over_selection_is_refused() -> None:
    """A filter wider than the class must fail, not pass by omission.

    Offering the check only class members makes `binary(*)` look correct,
    because every name it is asked about is matched. The non-trybuild
    binaries are what make the difference visible.
    """
    others = non_trybuild_binaries()
    assert others, (
        "this case needs at least one declared non-trybuild test binary; "
        "without one, over-selection cannot be observed at all and this "
        "test would pass vacuously"
    )
    expression = f"binary(*) | {' | '.join(f'binary({name})' for name in sorted(others))}"
    selected = binaries_selected_by(expression, sorted(trybuild_binaries() | others))
    chosen = {name for name, covered in selected.items() if covered}
    assert chosen - trybuild_binaries(), (
        "binary(*) selects every name offered, so the equality check in "
        "test_every_trybuild_binary_is_covered_by_the_override has something "
        "to reject"
    )


#: A directory whose mode denies a listing only denies one to a non-root
#: user, so a refusal cannot be observed when the suite runs as root.
ROOT_IS_NOT_DENIED = hasattr(os, "geteuid") and os.geteuid() == 0


def _nested_trybuild_tree(tmp_path: pathlib.Path) -> pathlib.Path:
    """Build a workspace whose trybuild call sits behind a nested ``mod.rs``.

    Only a ``[[test]]`` target declaring a ``mod.rs`` path makes a tree be
    walked rather than one file read, so the root is declared that way. The
    call is two levels down, which is the shape an unreadable directory in
    the middle would hide.

    Parameters
    ----------
    tmp_path : pathlib.Path
        The empty directory to build in.

    Returns
    -------
    pathlib.Path
        The repository root, with the crate beneath it.
    """
    crate = tmp_path / "ortho_config"
    crate.mkdir()
    (crate / "Cargo.toml").write_text(
        '[package]\nname = "ortho_config"\n\n'
        '[[test]]\nname = "nested_root"\npath = "tests/nested/mod.rs"\n',
        encoding="utf-8",
    )
    nested = crate / "tests" / "nested"
    nested.mkdir(parents=True)
    (nested / "mod.rs").write_text("mod inner;\n", encoding="utf-8")
    (nested / "inner").mkdir()
    (nested / "inner" / "mod.rs").write_text(
        "fn c() { trybuild::TestCases::new(); }\n", encoding="utf-8"
    )
    (tmp_path / "cargo-orthohelp").mkdir()
    return tmp_path


def test_a_nested_trybuild_call_is_still_found_through_the_walk(
    tmp_path: pathlib.Path,
) -> None:
    """The walk still reads the tree, and still finds what it found before.

    The nested source carries the only trybuild call, so a walk that
    stopped at the root, or that read the tree in some other way, would
    drop the name from the class here rather than in production.
    """
    root = _nested_trybuild_tree(tmp_path)
    assert trybuild_binaries(root) == frozenset({"nested_root"})
    assert non_trybuild_binaries(root) == frozenset()


@pytest.mark.skipif(ROOT_IS_NOT_DENIED, reason="root is not denied a listing")
def test_an_unreadable_nested_directory_is_refused_not_skipped(
    tmp_path: pathlib.Path,
) -> None:
    """A tree the walk cannot finish reading is a fault, not a short read.

    ``rglob`` passes an unlistable subdirectory over in silence, so the
    binary rooted above it left the trybuild class and the coverage
    assertion never asked for it. The fault has to name the directory it
    could not read, because that is the whole difference between this and
    the silence it replaces.
    """
    root = _nested_trybuild_tree(tmp_path)
    hidden = root / "ortho_config" / "tests" / "nested" / "inner"
    os.chmod(hidden, 0o000)
    try:
        with pytest.raises(ScanError) as raised:
            trybuild_binaries(root)
    finally:
        os.chmod(hidden, 0o755)
    assert str(hidden) in str(raised.value), raised.value
    assert isinstance(raised.value.__cause__, OSError), raised.value.__cause__


@pytest.mark.skipif(ROOT_IS_NOT_DENIED, reason="root is not denied a listing")
def test_an_unreadable_crate_root_is_refused_not_read_as_empty(
    tmp_path: pathlib.Path,
) -> None:
    """A crate that cannot be read is not a crate with no binaries.

    Answering ``{}`` for a ``tests/`` directory that exists and cannot be
    opened would report the crate as having no test binaries at all, which
    is the reading most likely to be mistaken for compliance.
    """
    root = _nested_trybuild_tree(tmp_path)
    hidden = root / "ortho_config" / "tests"
    os.chmod(hidden, 0o000)
    try:
        with pytest.raises(ScanError) as raised:
            trybuild_binaries(root)
    finally:
        os.chmod(hidden, 0o755)
    assert str(hidden) in str(raised.value), raised.value
