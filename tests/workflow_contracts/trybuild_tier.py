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

See "The trybuild allowance was raised after it killed two tests" in
``docs/developers-guide.md``.
"""

from __future__ import annotations

import fnmatch
import re
import typing as typ
from pathlib import Path

from timeout_budgets import REPO_ROOT

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

TEST_TARGET = re.compile(r"^\[\[test\]\]$", re.MULTILINE)
TEST_TARGET_NAME = re.compile(r'^name\s*=\s*"([^"]+)"', re.MULTILINE)
TEST_TARGET_PATH = re.compile(r'^path\s*=\s*"([^"]+)"', re.MULTILINE)

#: A glob this contract can evaluate. `globset` also accepts brace
#: alternation, which `fnmatch` does not, so a brace is refused rather than
#: misread: a filter using one would otherwise be judged by different rules
#: than nextest applies and could pass here while selecting something else.
GLOB_METACHARACTERS: typ.Final[str] = "*?["


class UnreadableMatcherError(Exception):
    """Raised when a ``binary(...)`` argument is not a matcher this reads.

    Every matcher in the nextest reference is read here, but a form added
    later would not be. Refusing is the safe direction: a matcher read as
    matching nothing would report the whole trybuild class as uncovered,
    and one read as matching everything would report a broken filter as
    sound. Neither is a guess worth making silently.
    """


def _declared_test_targets(manifest: str) -> dict[str, str]:
    """Return each ``[[test]]`` target's name, keyed by its source path.

    Parameters
    ----------
    manifest : str
        A crate's ``Cargo.toml`` text.

    Returns
    -------
    dict of str to str
        The declared binary name for each declared source path.
    """
    declared: dict[str, str] = {}
    # Split on the header so each block is read alone; a name in one block
    # and a path in another must not be paired.
    for block in TEST_TARGET.split(manifest)[1:]:
        name = TEST_TARGET_NAME.search(block)
        path = TEST_TARGET_PATH.search(block)
        if name is not None and path is not None:
            declared[path.group(1)] = name.group(1)
    return declared


def _crate_trybuild_binaries(base: Path, crate: str) -> set[str]:
    """Return the trybuild binary names one crate contributes.

    Split from :func:`trybuild_binaries` because the walk has two guards at
    different depths — the crate's ``tests/`` may be absent, and most files
    under it carry no trybuild call — and both belong to the crate's turn
    rather than to the workspace's. Keeping them together left the function
    with two nested conditional blocks, which is a CodeScene "Bumpy Road
    Ahead"; separating the per-crate reading is also what makes each half
    readable on its own.

    Parameters
    ----------
    base : Path
        The repository root.
    crate : str
        The crate's directory name within the workspace.

    Returns
    -------
    set of str
        The trybuild binary names this crate contributes, possibly none.

    Examples
    --------
    A crate with no ``tests/`` directory contributes nothing:

    >>> _crate_trybuild_binaries(Path("/nonexistent"), "ortho_config")
    set()
    """
    tests = base / crate / "tests"
    if not tests.is_dir():
        return set()
    manifest = (base / crate / "Cargo.toml").read_text(encoding="utf-8")
    declared = _declared_test_targets(manifest)
    return {
        declared.get(source.relative_to(base / crate).as_posix(), source.stem)
        for source in sorted(tests.glob("*.rs"))
        if TRYBUILD_CALL.search(source.read_text(encoding="utf-8")) is not None
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
    base = REPO_ROOT if root is None else root
    found: set[str] = set()
    for crate in CRATE_DIRECTORIES:
        found.update(_crate_trybuild_binaries(base, crate))
    return frozenset(found)


def declared_non_trybuild_binaries(root: Path | None = None) -> frozenset[str]:
    """Return every declared test binary that is not a trybuild one.

    A ``[[test]]`` target may live in a subdirectory, and
    :func:`trybuild_binaries` globs only ``tests/*.rs``, so these names
    are invisible to it in both directions. They are read here so the
    override's filter can be checked against binaries it must *not*
    select, which is the only way over-selection is visible: a filter
    naming a superset of the class passes a coverage check that offers
    it nothing but class members to match against.

    Parameters
    ----------
    root : Path, optional
        The repository root. Defaults to the one the other contracts use.

    Returns
    -------
    frozenset of str
        The declared binary names that carry no trybuild coverage.

    Examples
    --------
    A directory with no crates contributes nothing:

    >>> declared_non_trybuild_binaries(Path("/nonexistent"))
    frozenset()
    """
    base = REPO_ROOT if root is None else root
    trybuild = trybuild_binaries(base)
    found: set[str] = set()
    for crate in CRATE_DIRECTORIES:
        manifest = base / crate / "Cargo.toml"
        if not manifest.is_file():
            continue
        found.update(_declared_test_targets(manifest.read_text(encoding="utf-8")).values())
    return frozenset(found - trybuild)


def _matches_one(matcher: str, name: str) -> bool:
    """Report whether one ``binary(...)`` argument matches a binary name.

    The matchers are the ones in the nextest filterset reference. A bare
    argument, and a ``#``-prefixed one, are globs -- that is nextest's
    default for ``binary()``, so `binary(foo)` is an exact match only
    because `foo` holds no metacharacter.

    Parameters
    ----------
    matcher : str
        The argument of one ``binary(...)`` call, trimmed.
    name : str
        A binary name.

    Raises
    ------
    UnreadableMatcherError
        If the argument is not a matcher this contract reads.

    Returns
    -------
    bool
        Whether this argument selects that name.

    Examples
    --------
    >>> _matches_one("compile_fail", "compile_fail")
    True
    >>> _matches_one("*trybuild", "crate_path_trybuild")
    True
    >>> _matches_one("=compile_fail", "compile_fail")
    True
    >>> _matches_one("~fail", "compile_fail")
    True
    >>> _matches_one("/^compile_/", "compile_fail")
    True
    """
    match matcher[:1], matcher:
        case ("=", _):
            return matcher[1:] == name
        case ("~", _):
            return matcher[1:] in name
        case ("/", _):
            pattern = matcher[1:-1] if matcher.endswith("/") else None
            if pattern is None:
                raise UnreadableMatcherError(f"unterminated regex matcher {matcher!r}")
            return re.search(pattern, name) is not None
        case ("#", _):
            pattern = matcher[1:]
        case _:
            pattern = matcher
    # `globset` reads `{a,b}` alternation and `fnmatch` does not, so the
    # difference is refused rather than left to disagree with nextest.
    if "{" in pattern:
        raise UnreadableMatcherError(
            f"glob {matcher!r} uses brace alternation, which this contract "
            f"cannot evaluate as nextest does"
        )
    return fnmatch.fnmatchcase(name, pattern)


BINARY_TERM = re.compile(r"binary\(([^)]*)\)")

#: What may sit between ``binary(...)`` terms for the disjunction to be
#: read as one. Anything else — `&`, `-`, `not(...)`, or a `test(...)`
#: term — changes the selection by narrowing it, so it is refused rather
#: than dropped on the floor.
BINARY_DISJUNCTION = re.compile(r"^[\s|]*$")


def binaries_selected_by(filter_expression: str, candidates: typ.Iterable[str]) -> dict[str, bool]:
    """Return which candidates a disjunction of ``binary`` terms selects.

    Only a ``|``-joined list of ``binary(...)`` predicates is read. That
    is the shape the trybuild override uses, and it is the only shape in
    which the `binary` terms decide the answer on their own: a
    conjunction, a difference, or a negation narrows the selection in a
    way this reader does not evaluate — `binary(compile_fail) &
    test(other)` selects nothing when `other` matches no test, while
    rating the `binary` term alone would report `compile_fail` as
    covered. Such an expression is refused rather than misread.

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
    terms = BINARY_TERM.findall(filter_expression)
    if not terms:
        message = (
            f"the trybuild override's filter names no binary(...) predicate: "
            f"{filter_expression!r}"
        )
        raise UnreadableMatcherError(message)
    # Removing the terms leaves the glue between them. Checking it here,
    # rather than scanning the whole expression, is what keeps an
    # operator inside a matcher — a regex or glob such as `binary(/a-b/)`
    # — from being refused as a stray operator.
    remainder = BINARY_TERM.sub("", filter_expression)
    if BINARY_DISJUNCTION.match(remainder) is None:
        message = (
            f"the trybuild override's filter combines its binary(...) terms "
            f"with something other than `|`, which this contract does not "
            f"evaluate; reading the terms alone would report a narrower "
            f"expression as a wider one: {filter_expression!r}"
        )
        raise UnreadableMatcherError(message)
    return {
        name: any(_matches_one(term.strip(), name) for term in terms)
        for name in candidates
    }
