"""Reading which test binaries drive a child `cargo` of their own.

A binary whose tests spawn `cargo` pays a nested build inside the enclosing
test's wall clock: compiler admission, the package-cache lock, and a cold
dependency graph are all on that test's timer. That is a different cost from
an ordinary test's, which is why the class is budgeted apart. It was not
until issue #611: the heaviest binaries in the file sat on the base
allowance, and the only raised one belonged to the trybuild class -- whose
child build a trybuild binary *is*, rather than spawns.

Membership is a fact about the Rust sources, so it is read from them and
compared with what the filter selects, as ``trybuild_tier`` does for its own
class. Two mistakes are avoided in the reading:

**A marker grep over the `tests/` tree is wrong in both directions.**
Grepping ``Command::new("cargo")`` also matches ``clap::Command::new`` and
``Command::new(executable)`` in a probe that re-runs the test binary, and it
matches every file that merely mentions a helper. Membership is therefore
decided by the module graph: a binary is a member when a file *reachable from
its root* carries a marker, so a shared helper makes every binary that
declares it a member, and no other.

**``#[path]`` makes declaration names irrelevant.** ``#[path =
"test_utils.rs"] mod other_name;`` is legal and this repository uses it, so a
rule keyed on the declared name would link a binary to a file it never
compiles.

The rules that turn a declaration into a file live in
:mod:`rust_module_graph`; the child-cargo marker patterns live in
:mod:`child_cargo_source_markers`. This module inventories the audited test
binaries and combines those source readings into the class. The split is the
one :mod:`nextest_filterset` made from :mod:`trybuild_tier`: this module knows
the class and not the filter language, that one the reverse.

See "Test timeouts: the tiers this repository sets" in
``docs/developers-guide.md``.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

from child_cargo_source_markers import (
    CARGO_MARKERS,
    ScanError,
    carries_child_cargo,
    marker_sources,
)
from nextest_filterset import (
    UnreadableMatcherError,
    binaries_selected_by as _binaries_selected_by,
)
from source_scan import (
    ManifestError,
    declared_test_targets,
    exists,
    is_directory,
    listing,
    scan,
)

__all__ = [
    "CARGO_MARKERS",
    "CHILD_CARGO_TIER_CRATES",
    "ManifestError",
    "ScanError",
    "UnreadableMatcherError",
    "binaries_selected_by",
    "carries_child_cargo",
    "child_cargo_binaries",
    "marker_binaries",
    "non_child_cargo_binaries",
]

#: The crates whose test binaries are audited. Both spawn the built
#: ``cargo-orthohelp`` or ``cargo`` itself. ``hello_world`` is left out: its
#: behaviour harness runs an already-built example rather than invoking cargo,
#: so auditing it would only invite a false positive.
CHILD_CARGO_TIER_CRATES: typ.Final[tuple[str, ...]] = ("ortho_config", "cargo-orthohelp")


def _discovered_test_roots(
    crate_root: Path, declared: dict[str, str]
) -> list[tuple[str, Path]]:
    """Return the test roots Cargo finds on its own, by name and root file.

    Cargo auto-discovers both a top-level ``tests/*.rs`` file and a
    ``tests/<name>/main.rs`` directory target, which is a test target named
    for its directory. Omitting the directory form leaves such a binary out of
    the inventory, so a child-cargo target with a new name could stay on the
    base allowance without failing the coverage assertion.

    A root a ``[[test]]`` target declares is skipped here and added by the
    caller, so the two enumerations cannot double-count one binary.

    Examples
    --------
    A crate with no ``tests`` directory discovers nothing:

    >>> _discovered_test_roots(Path("/nonexistent"), {})
    []
    """
    tests = crate_root / "tests"
    if not scan(tests, exists(tests)):
        return []
    entries = scan(tests, listing(tests))
    roots = [
        (source.stem, source)
        for source in entries
        if source.name.endswith(".rs")
        and source.relative_to(crate_root).as_posix() not in declared
    ]
    roots.extend(
        (child.name, child / "main.rs")
        for child in entries
        if scan(child, is_directory(child))
        and (child / "main.rs").relative_to(crate_root).as_posix() not in declared
        and scan(child / "main.rs", exists(child / "main.rs"))
    )
    return roots


def _crate_test_roots(base: Path, crate: str) -> list[tuple[str, Path]]:
    """Return one crate's test binaries as pairs of name and crate root.

    Every kind Cargo builds is enumerated: a ``[[test]]`` target, a top-level
    ``tests/*.rs`` file, and a ``tests/<name>/main.rs`` directory target. An
    inventory holding only the declared ones offers the coverage assertion a
    universe missing almost every binary, so a filter naming the whole class
    would pass an equality check it should fail.

    The crate's ``tests/`` directory gates the discovery walk and nothing
    else, because a ``[[test]]`` target carries its own ``path`` and may sit
    anywhere in the crate.

    The name is the file stem, the directory name, or the declared target
    name, which is what a nextest ``binary(...)`` filter matches; the path is
    what the module graph is walked from.

    Examples
    --------
    A directory with no crate contributes nothing:

    >>> _crate_test_roots(Path("/nonexistent"), "ortho_config")
    []
    """
    crate_root = base / crate
    manifest = crate_root / "Cargo.toml"
    declared = (
        declared_test_targets(
            scan(manifest, manifest.read_text, encoding="utf-8"), manifest
        )
        if scan(manifest, exists(manifest))
        else {}
    )
    roots = _discovered_test_roots(crate_root, declared)
    roots.extend((name, crate_root / path) for path, name in sorted(declared.items()))
    return [(name, source) for name, source in roots if scan(source, exists(source))]


def _crate_test_binaries(base: Path, crate: str) -> dict[str, bool]:
    """Return each of one crate's test binaries, marked by child-cargo use.

    Examples
    --------
    A directory with no crate contributes nothing:

    >>> _crate_test_binaries(Path("/nonexistent"), "ortho_config")
    {}
    """
    return {
        name: carries_child_cargo(source)
        for name, source in _crate_test_roots(base, crate)
    }


def _workspace_test_binaries(base: Path) -> dict[str, bool]:
    """Return every audited binary, marked by child-cargo use.

    A name is **workspace-wide**, not per-crate: ``binary(...)`` matches a
    bare name with no crate prefix, so ``rstest_bdd`` selects both
    ``ortho_config::rstest_bdd`` and ``cargo-orthohelp::rstest_bdd``. The
    entry is true when *any* crate's copy carries a marker, which is what the
    filter's selection covers; last-write-wins merging would let a plain copy
    overwrite a member and drop it from the class.

    Examples
    --------
    A directory with no crates has no test binaries:

    >>> _workspace_test_binaries(Path("/nonexistent"))
    {}
    """
    found: dict[str, bool] = {}
    for crate in CHILD_CARGO_TIER_CRATES:
        for name, carries in _crate_test_binaries(base, crate).items():
            found[name] = found.get(name, False) or carries
    return found


def marker_binaries(base: Path) -> dict[str, frozenset[str]]:
    """Return, per marker label, the binaries whose sources carry it.

    The union of the values is the class :func:`child_cargo_binaries` returns,
    but the split is reported because a label that has gone dead is invisible
    from the union: it contributes nothing, so the class shrinks and the
    coverage assertion agrees with a filter that also shrank. Naming the empty
    group is what makes that visible.

    Membership is read the same way as the class's -- from the files each
    binary compiles, not from the text of the ``tests/`` tree -- so a marker
    living in a shared helper is attributed to every binary that reaches it.

    Parameters
    ----------
    base : pathlib.Path
        The repository root.

    Returns
    -------
    dict of str to frozenset of str
        One entry per label in :data:`CARGO_MARKERS`. An empty frozenset means
        the label matched nothing anywhere in the audited crates.

    Raises
    ------
    ScanError
        If a source a binary compiles cannot be read.

    Examples
    --------
    A directory with no crates has a group per label, all empty:

    >>> sorted(marker_binaries(Path("/nonexistent")))
    ['binary-loader', 'cargo-runner', 'cargo-spawn']
    """
    found: dict[str, set[str]] = {label: set() for label, _ in CARGO_MARKERS}
    for crate in CHILD_CARGO_TIER_CRATES:
        for name, source in _crate_test_roots(base, crate):
            for label in marker_sources(source):
                found[label].add(name)
    return {label: frozenset(names) for label, names in found.items()}


def _binaries_where(base: Path, *, carries: bool) -> frozenset[str]:
    """Return every audited binary whose child-cargo use is ``carries``.

    Both halves of the partition are read here so they cannot disagree about
    which binaries exist: :func:`child_cargo_binaries` asks for the carrying
    ones and :func:`non_child_cargo_binaries` for the rest, and the two calls
    are the same walk with one predicate inverted.

    Parameters
    ----------
    base : pathlib.Path
        The repository root.
    carries : bool
        Which half to return.

    Returns
    -------
    frozenset of str
        The matching binary names.

    Raises
    ------
    ScanError
        If a source a binary compiles cannot be read.

    Examples
    --------
    A directory with no crates contributes nothing:

    >>> _binaries_where(Path("/nonexistent"), carries=True)
    frozenset()
    """
    return frozenset(
        name
        for name, found in _workspace_test_binaries(base).items()
        if found is carries
    )


def child_cargo_binaries(base: Path) -> frozenset[str]:
    """Return the name of every test binary that drives a child cargo.

    The name is what a nextest ``binary(...)`` filter matches, so the set is
    directly comparable with the one the configuration selects.

    Parameters
    ----------
    base : pathlib.Path
        The repository root.

    Returns
    -------
    frozenset of str
        The binary names that spawn a child ``cargo``. Empty when ``base``
        holds no audited crate.

    Raises
    ------
    ScanError
        If a source a binary compiles cannot be read.

    Examples
    --------
    A directory with no crates contributes nothing:

    >>> child_cargo_binaries(Path("/nonexistent"))
    frozenset()
    """
    return _binaries_where(base, carries=True)


def non_child_cargo_binaries(base: Path) -> frozenset[str]:
    """Return every audited binary that does not drive a child cargo.

    The set makes over-selection visible: a filter naming a superset of the
    class passes a coverage assertion offered nothing but class members. Both
    halves are read by :func:`_workspace_test_binaries`, so they cannot
    disagree about which binaries exist.

    Parameters
    ----------
    base : pathlib.Path
        The repository root.

    Returns
    -------
    frozenset of str
        The binary names that spawn no child ``cargo``. Empty when ``base``
        holds no audited crate.

    Raises
    ------
    ScanError
        If a source a binary compiles cannot be read.

    Examples
    --------
    A directory with no crates contributes nothing:

    >>> non_child_cargo_binaries(Path("/nonexistent"))
    frozenset()
    """
    return _binaries_where(base, carries=False)


def binaries_selected_by(
    filter_expression: str, candidates: typ.Iterable[str]
) -> dict[str, bool]:
    """Return which candidates a disjunction of ``binary`` terms selects.

    Re-exported from :mod:`nextest_filterset`, which reads the language, so
    the contract has one import site for the class and its filter.

    Parameters
    ----------
    filter_expression : str
        A nextest filterset, as a configuration's ``filter`` key holds it.
    candidates : iterable of str
        The binary names to test.

    Returns
    -------
    dict of str to bool
        Each candidate's selection, in the order given.

    Raises
    ------
    UnreadableMatcherError
        If the expression is not a ``binary`` disjunction, names no
        ``binary(...)`` predicate, or passes a ``binary(...)`` argument
        using a matcher this contract cannot evaluate.

    Examples
    --------
    >>> binaries_selected_by('binary(rstest_bdd)', ["rstest_bdd", "other"])
    {'rstest_bdd': True, 'other': False}
    """
    return _binaries_selected_by(filter_expression, candidates)
