"""Reading which test binaries need the trybuild per-test allowance.

A trybuild binary spawns a child `cargo` in its own target directory, so it
pays a cold dependency build that no other test here pays, and it contends
with its neighbours on one package cache. That is why the class is
budgeted apart, and why its allowance is the largest in the file.

Which binaries belong to the class is a fact about the Rust sources, not
about the configuration file, so it is read from the sources and compared
with what the filter selects. Reading both ends is what makes the gap
visible: the filter named two binaries while seven existed, and nothing
noticed until one of the five omitted was killed at the allowance it should
never have been on.

The filesystem reads live in :mod:`source_scan`, which refuses a source it
cannot read rather than passing over it silently. That refusal is what makes
the class complete: a source skipped in silence is a binary left unasked for.

See "The trybuild allowance was raised after it killed two tests" in
``docs/developers-guide.md``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

from nextest_filterset import (
    UnreadableMatcherError,
    binaries_selected_by as _binaries_selected_by,
)
from source_scan import (
    ManifestError,
    ScanError,
    declared_test_targets,
    exists,
    listing,
    scan,
    sources_under,
)
from timeout_budgets import REPO_ROOT

#: Re-exported so the coverage contract keeps one import site for the
#: class it reads here and the language it reads there.
__all__ = [
    "ManifestError",
    "ScanError",
    "UnreadableMatcherError",
    "binaries_selected_by",
    "non_trybuild_binaries",
    "trybuild_binaries",
]

#: The call that identifies a trybuild test binary. Matched rather than
#: imported because the Rust sources cannot be executed here. Every
#: trybuild binary here writes this call in its test function, whether it
#: binds `TestCases` to a variable first or not.
#:
#: Matched as a pattern rather than a literal because Rust accepts
#: whitespace before the argument list, and a source written `new ()`
#: would otherwise be read as no trybuild binary at all — dropping it
#: from the class silently, so the coverage assertion would not ask for
#: it. `make fmt` normalises the spelling, but the miss would be a hole
#: in the class for exactly as long as it took to notice.
TRYBUILD_CALL: typ.Final[re.Pattern[str]] = re.compile(r"trybuild::TestCases::new\s*\(")

#: Where integration tests live, per crate. A binary's name is the test
#: file's stem unless a `[[test]]` target renames it.
CRATE_DIRECTORIES: typ.Final[tuple[str, ...]] = ("ortho_config", "cargo-orthohelp")

#: Re-exported for the coverage contract, which names the class's marker
#: whether or not it also reads the reading primitives behind it.
__all__ += ["TRYBUILD_CALL"]


def _carries_trybuild(root: Path) -> bool:
    """Report whether compiling the binary rooted at ``root`` runs trybuild.

    A declared ``mod.rs`` root compiles every source beneath it, because
    the modules it declares pull them in, so the whole tree is scanned
    rather than the root alone. A trybuild call in any of them is compiled
    into that one binary, which is why the scan is over the tree and not
    over the root: a ``mod.rs`` is usually a few ``mod`` lines, and a
    nested declaration's trybuild call would otherwise be invisible.

    The tree is an over-approximation of the module graph — it does not
    follow ``mod`` declarations, and ``#[path]`` can pull in a source
    outside it — but over-approximating is the correct direction here: a
    trybuild call that is not reachable costs the filter an entry it did
    not need, while one that is reachable and missed hides a binary that
    runs compile tests on the base allowance.

    Parameters
    ----------
    root : Path
        The file a test binary is rooted at.

    Returns
    -------
    bool
        Whether a trybuild call is compiled into that binary.

    Examples
    --------
    A source with no trybuild call does not carry one:

    >>> _carries_trybuild(Path("/nonexistent.rs"))
    False
    """
    sources = sources_under(root.parent) if root.name == "mod.rs" else [root]
    return any(
        scan(source, exists(source))
        and TRYBUILD_CALL.search(scan(source, source.read_text, encoding="utf-8"))
        is not None
        for source in sources
    )


def _crate_test_binaries(base: Path, crate: str) -> dict[str, bool]:
    """Return each of one crate's test binaries, marked by trybuild use.

    A binary is either declared by a ``[[test]]`` target or discovered by
    cargo from a top-level ``tests/*.rs`` file, and **both** kinds are
    enumerated. An inventory holding only the declared ones offers the
    coverage assertion a universe missing almost every binary, so a filter
    naming the whole trybuild class plus an undiscovered test would pass
    the equality check: over-selection is only visible against the
    binaries the check is given.

    The crate's ``tests/`` directory gates the walk that discovers
    undeclared binaries, not the reading of declared ones. Cargo scans
    that directory to find a top-level ``tests/*.rs`` file, but a
    ``[[test]]`` target carries its own ``path``, which may sit anywhere
    in the crate; guarding both on the directory dropped a declared
    target from the inventory whenever the directory was absent.

    Split from the two readers below because the walk has two guards at
    different depths — the crate's ``tests/`` may be absent, and a binary
    may be unreachable — and both belong to the crate's turn rather than
    to the workspace's. Keeping them together left the function with two
    nested conditional blocks, which is a CodeScene "Bumpy Road Ahead".

    Parameters
    ----------
    base : Path
        The repository root.
    crate : str
        The crate's directory name within the workspace.

    Returns
    -------
    dict of str to bool
        Each binary's name, mapped to whether it carries a trybuild call.

    Examples
    --------
    A crate that is not there has no test binaries:

    >>> _crate_test_binaries(Path("/nonexistent"), "ortho_config")
    {}
    """
    # The manifest is read whether or not the crate has a ``tests/``
    # directory, because that directory governs cargo's *directory scan*
    # and nothing else: a ``[[test]]`` target may name a path anywhere in
    # the crate, so a crate with no ``tests/`` at all can still declare
    # one. Guarding this read on that directory dropped such a target from
    # the inventory, and a binary missing from the inventory is one the
    # coverage assertion never asks about. A manifest that is merely
    # absent is an answer -- the crate declares no targets -- while every
    # other failure is still a refusal naming the path. The presence test
    # is `exists` rather than `Path.is_file`, which since 3.14 reports an
    # inaccessible path as absent; see `source_scan.exists`.
    manifest_path = base / crate / "Cargo.toml"
    declared = (
        declared_test_targets(
            scan(manifest_path, manifest_path.read_text, encoding="utf-8"),
            manifest_path,
        )
        if scan(manifest_path, exists(manifest_path))
        else {}
    )
    # A declared path is enumerated once, as its target; listing it as well
    # would offer the same binary under its stem and its declared name. It
    # is listed rather than globbed because `glob` passes over a directory
    # it cannot open in silence, the very miss this module now refuses.
    tests = base / crate / "tests"
    roots: list[tuple[str, Path]] = (
        [
            (source.stem, source)
            for source in scan(tests, listing(tests))
            if source.name.endswith(".rs")
            and source.relative_to(base / crate).as_posix() not in declared
        ]
        if scan(tests, exists(tests))
        else []
    )
    roots.extend((name, base / crate / path) for path, name in sorted(declared.items()))
    return {
        name: _carries_trybuild(source)
        for name, source in roots
        if scan(source, exists(source))
    }


def trybuild_binaries(root: Path | None = None) -> frozenset[str]:
    """Return the name of every trybuild test binary in the workspace.

    The name is what a nextest ``binary(...)`` filter matches, so the set is
    directly comparable with the one the configuration selects. A file under
    ``tests/`` declared as a ``[[test]]`` target takes that target's name;
    every other takes its file stem, which is the rule cargo applies.

    Parameters
    ----------
    root : Path, optional
        The repository root. Defaults to the one the other contracts use.

    Returns
    -------
    frozenset of str
        The binary names carrying trybuild coverage.

    Examples
    --------
    A directory with no crates contributes nothing:

    >>> trybuild_binaries(Path("/nonexistent"))
    frozenset()
    """
    return frozenset(
        name for name, carries in _workspace_test_binaries(root).items() if carries
    )


def _workspace_test_binaries(root: Path | None = None) -> dict[str, bool]:
    """Return every test binary in the audited crates, marked by trybuild use.

    Read by both :func:`trybuild_binaries` and
    :func:`non_trybuild_binaries` so the two halves of the coverage
    assertion cannot disagree about which binaries exist: a name absent
    from one and present in the other is how a class member came to be
    certified as covered while running on the base allowance.

    A name is **workspace-wide**, not per-crate: nextest's ``binary(...)``
    matches it with no crate prefix, so one name built in two crates is
    one class member the filter selects in both. The entry is therefore
    true when *any* crate's copy carries a trybuild call. Merging the
    per-crate readings last-write-wins let a later crate's plain copy
    overwrite an earlier crate's trybuild one, which dropped the name
    from the class and had the coverage assertion certify it as covered
    while it ran on the base allowance.

    Parameters
    ----------
    root : Path, optional
        The repository root. Defaults to the one the other contracts use.

    Returns
    -------
    dict of str to bool
        Each binary's name, mapped to whether it carries a trybuild call.

    Examples
    --------
    A directory with no crates has no test binaries:

    >>> _workspace_test_binaries(Path("/nonexistent"))
    {}
    """
    base = REPO_ROOT if root is None else root
    found: dict[str, bool] = {}
    for crate in CRATE_DIRECTORIES:
        for name, carries in _crate_test_binaries(base, crate).items():
            found[name] = found.get(name, False) or carries
    return found


def non_trybuild_binaries(root: Path | None = None) -> frozenset[str]:
    """Return every test binary in the audited crates that is not trybuild.

    These are the names the override's filter is checked against, and the
    check is only as strong as this set is complete: a filter naming a
    superset of the trybuild class passes a coverage assertion that offers
    it nothing but class members to match against. Cargo discovers a
    top-level ``tests/*.rs`` file as a binary without any ``[[test]]``
    entry, so reading only the declared targets would offer three names
    where fifty-odd exist, and an over-selection onto any of the others
    would be invisible.

    Parameters
    ----------
    root : Path, optional
        The repository root. Defaults to the one the other contracts use.

    Returns
    -------
    frozenset of str
        The binary names that carry no trybuild coverage.

    Examples
    --------
    A directory with no crates contributes nothing:

    >>> non_trybuild_binaries(Path("/nonexistent"))
    frozenset()
    """
    return frozenset(
        name for name, carries in _workspace_test_binaries(root).items() if not carries
    )


def binaries_selected_by(filter_expression: str, candidates: typ.Iterable[str]) -> dict[str, bool]:
    """Return which candidates a disjunction of ``binary`` terms selects.

    Re-exported from :mod:`nextest_filterset`, which reads the language
    itself. Kept importable here because the coverage contract reads the
    class from this module and the filter from that one, and one import
    site per module is easier to follow than two.

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
    >>> binaries_selected_by('binary(*trybuild)', ["a_trybuild", "other"])
    {'a_trybuild': True, 'other': False}
    """
    return _binaries_selected_by(filter_expression, candidates)
