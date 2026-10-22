"""Which Rust sources a test binary compiles, resolved from its root.

A test binary's dependence on a shared helper is not visible in its own text:
``mod fixtures;`` names a declaration, and which file that name reaches is
decided by the Reference's rules rather than by anything the repository
writes down. Reading membership by grepping the ``tests/`` tree is therefore
wrong in both directions -- it matches files a binary never compiles, and
misses the helpers it does -- so the graph is walked instead.

The rules, each of which this repository exercises:

- A crate root (``lib.rs``, ``main.rs``) and a ``mod.rs`` are their own module
  directory; any other ``foo.rs`` resolves names from ``foo/``.
- ``#[path = "..."]`` is relative to the *declaring file's* directory and may
  contain ``..``. It makes the declared name irrelevant, since ``#[path =
  "test_utils.rs"] mod other_name;`` is legal and is used here.
- A file reached through ``#[path]`` keeps its siblings as they are; it does
  not gain a ``foo/`` directory of its own.
- Every ``tests/*.rs`` is a crate root, whatever it is called. Being one is a
  fact about how the file is compiled, not about its name, and a rule keyed on
  the name alone silently drops the binaries whose roots are not called
  ``mod.rs``. That rule gave ``tests/documentation_examples_tests.rs`` a module
  directory of ``tests/documentation_examples_tests/`` and resolved nothing,
  while the modules it declares live in ``tests/documentation_examples/``,
  shared by three binaries that each declare them.

One shape is over-approximated rather than guessed at. An inline ``mod outer {
... }`` contributes a directory component no file text reveals, so
:func:`_search_one_level` takes one level of directory search in place of it.
That can find a same-named file the compiler would not; the safe direction is
a false positive, which puts a binary on a raised tier it does not need,
against a false negative, which leaves a binary on the base allowance while it
spawns a child build of its own.

This module reads the *language* and knows nothing of any test class, for the
reason :mod:`nextest_filterset` is kept apart from :mod:`trybuild_tier`: each
stays legible, and neither outgrows the file-length limit ``AGENTS.md`` sets.
Its only caller is :mod:`child_cargo_tier`. A third class reader that also
needs module resolution should import :func:`sources_reached` rather than
grow a second copy of these rules.
"""

from __future__ import annotations

import posixpath
import re
import stat
import typing as typ
from pathlib import Path

from source_scan import ScanError, listing, scan

__all__ = ["ScanError", "module_directory", "sources_reached"]

#: A ``mod name;`` declaration, with the attributes written above it captured.
#: The attribute group pairs ``#[path]`` with its declaration across the
#: newline rustfmt puts between them; a pattern demanding adjacency missed
#: every ``#[path]`` in the repository.
_MODULE = re.compile(
    r"^[ \t]*(?P<attributes>(?:#\[(?:[^\[\]]|\[[^\[\]]*\])*\][ \t]*\r?\n[ \t]*)*)"
    r"(?:pub(?:[ \t]*\([^)]*\))?[ \t]+)?mod[ \t]+(?P<name>[A-Za-z_][A-Za-z0-9_]*)[ \t]*;",
    re.MULTILINE,
)

_PATH_ATTRIBUTE = re.compile(r'#\[[ \t]*path[ \t]*=[ \t]*"(?P<path>[^"]+)"[ \t]*\]')

#: The names a crate root can have. A root file is its own module directory.
_ROOT_NAMES: typ.Final[frozenset[str]] = frozenset({"lib.rs", "main.rs", "mod.rs"})


def _mode(path: Path) -> int | None:
    """Return ``path``'s mode, or ``None`` when it is not there."""
    try:
        return path.stat().st_mode
    except FileNotFoundError:
        return None
    except OSError as error:
        message = f"{path} could not be read: {error}"
        raise ScanError(message) from error


def _is_file(path: Path) -> bool:
    """Report whether ``path`` is a regular file.

    ``Path.is_file`` is not usable for this for the reason
    :func:`source_scan.exists` gives: since 3.14 it answers ``False`` for any
    ``OSError``, so an unreadable directory reads as a missing module and the
    sources behind it are left unasked for.

    Examples
    --------
    >>> _is_file(Path(__file__))
    True
    >>> _is_file(Path("/nonexistent.rs"))
    False
    """
    mode = _mode(path)
    return mode is not None and stat.S_ISREG(mode)


def _is_directory(path: Path) -> bool:
    """Report whether ``path`` is a directory."""
    mode = _mode(path)
    return mode is not None and stat.S_ISDIR(mode)


def module_directory(source: Path, *, resolves_from_parent: bool) -> Path:
    """Return the directory ``source`` resolves its ``mod`` names from.

    ``resolves_from_parent`` is true for a crate root and for a file reached
    through ``#[path]``: both resolve their declarations beside themselves.

    It is keyword-only because the two answers for the same file differ, so a
    bare ``True`` or ``False`` at a call site says nothing about which rule is
    being applied.

    Parameters
    ----------
    source : pathlib.Path
        The file whose ``mod`` declarations are being resolved.
    resolves_from_parent : bool
        Whether ``source`` resolves its declarations beside itself, as a crate
        root or a ``#[path]`` target does, rather than from ``<stem>/``.

    Returns
    -------
    pathlib.Path
        The directory its declarations are resolved against.

    Examples
    --------
    A non-root file owns a directory named for its stem; a root does not:

    >>> module_directory(Path("tests/fixtures/mod.rs"), resolves_from_parent=False)
    PosixPath('tests/fixtures')
    >>> module_directory(Path("tests/behaviour.rs"), resolves_from_parent=False)
    PosixPath('tests/behaviour')
    >>> module_directory(Path("tests/behaviour.rs"), resolves_from_parent=True)
    PosixPath('tests')
    """
    if resolves_from_parent or source.name in _ROOT_NAMES:
        return source.parent
    return source.parent / source.stem


def _search_one_level(directory: Path, name: str) -> list[Path]:
    """Return matches for a declaration inside an inline module block."""
    if not _is_directory(directory):
        # A module directory that is not there has no children to search, and
        # `listing` raises rather than answering empty for one.
        return []
    children = [
        candidate
        for candidate in scan(directory, listing(directory))
        if _is_directory(candidate)
    ]
    return [
        candidate
        for child in children
        for candidate in (child / f"{name}.rs", child / f"{name}" / "mod.rs")
        if _is_file(candidate)
    ]


def _resolve(directory: Path, source: Path, name: str, attribute: str | None) -> list[Path]:
    """Return the files a ``mod`` declaration could name.

    More than one answer is a fault the caller reports rather than a choice
    made here: ``foo.rs`` and ``foo/mod.rs`` may not both exist, and silently
    preferring one would let membership depend on which spelling is on disk.

    Examples
    --------
    A missing module resolves to nothing rather than raising:

    >>> _resolve(Path("/nonexistent"), Path("/nonexistent.rs"), "child", None)
    []
    """
    if attribute is not None:
        # Relative to the declaring file's directory; `normpath` collapses the
        # `..` the repository's `#[path]` values use.
        named = Path(posixpath.normpath(str(source.parent / attribute)))
        return [named] if _is_file(named) else []
    direct = [directory / f"{name}.rs", directory / f"{name}" / "mod.rs"]
    found = [candidate for candidate in direct if _is_file(candidate)]
    return found or _search_one_level(directory, name)


def _declarations(text: str) -> typ.Iterator[tuple[str, str | None]]:
    """Yield each file-loading ``mod`` in ``text`` with its ``#[path]``."""
    for declaration in _MODULE.finditer(text):
        attribute = _PATH_ATTRIBUTE.search(declaration.group("attributes") or "")
        yield declaration.group("name"), (attribute.group("path") if attribute else None)


def sources_reached(root: Path) -> dict[Path, str]:
    """Return every source ``root`` compiles, mapped to its text.

    A ``mod`` written with a body is not followed: the pattern requires the
    semicolon that ends a file-loading declaration. Inline blocks have no file
    of their own, and their bodies are reached by :func:`_search_one_level`
    instead.

    The text is returned alongside the path so a caller that looks for a
    marker does not read every file a second time.

    Parameters
    ----------
    root : pathlib.Path
        The crate root of one test binary.

    Returns
    -------
    dict of pathlib.Path to str
        Each file the binary compiles, mapped to the text read from it. Empty
        when ``root`` is not there.

    Raises
    ------
    ScanError
        If a source cannot be read, or a ``mod`` declaration resolves to more
        than one file.

    Examples
    --------
    A root that is not there compiles nothing:

    >>> sources_reached(Path("/nonexistent.rs"))
    {}
    """
    reached: dict[Path, str] = {}
    seen: set[Path] = set()
    # The root is a crate root, so it resolves its declarations beside itself.
    pending: list[tuple[Path, bool]] = [(root, True)]
    while pending:
        source, resolves_from_parent = pending.pop()
        if source in seen or not _is_file(source):
            continue
        seen.add(source)
        text = scan(source, source.read_text, encoding="utf-8")
        reached[source] = text
        directory = module_directory(source, resolves_from_parent=resolves_from_parent)
        for name, attribute in _declarations(text):
            resolved = _resolve(directory, source, name, attribute)
            if len(resolved) > 1:
                message = (
                    f"{source} declares `mod {name};`, which resolves to more "
                    f"than one file: {[str(path) for path in resolved]}"
                )
                raise ScanError(message)
            pending.extend((path, attribute is not None) for path in resolved)
    return reached
