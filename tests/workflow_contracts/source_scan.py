"""Reading Rust sources and manifests, refusing a read that fails.

Every filesystem read behind the trybuild contract goes through here, so
that a source which cannot be read is refused rather than passed over. The
distinction matters: a source that is skipped silently is a binary that
leaves the trybuild class unasked for, and the coverage assertion in
:mod:`trybuild_tier` is only as strong as this reading.

Split from :mod:`trybuild_tier` so that module stays within the
repository's 400-line code file ceiling. The functions here are a pure
move: the reading primitives are one concern, and computing which binaries
carry a trybuild call is another.
"""

from __future__ import annotations

import os
import tomllib
import typing as typ
from pathlib import Path


class ScanError(OSError):
    """Raised when a source this module reads cannot be read.

    The message names the path that could not be read, and the original
    failure is kept as ``__cause__``. Raised rather than passed over,
    because a source that is not read is a binary that may leave the
    class unasked for, and the assertion is only as strong as this reading.
    """


def sources_under(directory: Path) -> typ.Iterator[Path]:
    """Yield every ``*.rs`` file at or beneath ``directory``, refusing a miss.

    A directory the walk cannot open is refused rather than skipped.
    ``rglob`` reaches ``os.walk`` with no ``on_error`` and ``Path.walk``
    delegates to it the same way, so an unreadable subdirectory used to be
    passed over in silence, and the binary rooted there left the class
    unasked for. The walk stays lazy, so a match short-circuits the caller.

    Parameters
    ----------
    directory : Path
        The directory to walk.

    Yields
    ------
    Path
        Each ``*.rs`` file beneath it, refusing an unlistable one.
    """

    def refuse(error: OSError) -> typ.NoReturn:
        """Refuse a walk directory that could not be listed, naming it."""
        at = Path(error.filename or directory)
        raise ScanError(f"{at} could not be read: {error}") from error

    for at, _directories, filenames in os.walk(directory, onerror=refuse):
        yield from (Path(at, name) for name in filenames if name.endswith(".rs"))


def scan(
    path: Path, read: typ.Callable[..., typ.Any], *args: typ.Any, **kwargs: typ.Any
) -> typ.Any:
    """Call one ``Path`` read, refusing a failure that names no path.

    Every filesystem read here goes through this: ``path`` is the path
    the call is over and the one a failure names, and ``read`` is the
    call to make -- a bound ``Path`` method such as ``Path.read_text``,
    or a deferred callable such as :func:`listing`. ``is_file`` and
    ``is_dir`` answer ``False`` for a path that is merely absent, and
    ``False`` is an answer rather than a refusal; any other failure is a
    :class:`ScanError` naming ``path``, the original as ``__cause__``.

    The callable form is not a convenience. ``read(*args)`` evaluates its
    arguments *before* the ``try`` below, and ``Path.iterdir`` starts its
    ``os.scandir`` at a different moment depending on the interpreter:
    lazily from a generator body on 3.12, eagerly on 3.14. Passing
    ``list`` and ``tests.iterdir()`` therefore guarded the read on one
    interpreter and left the raise uncaught on the other. Deferring the
    whole read -- enumerate and materialize together, as :func:`listing`
    does -- is version-independent; testing it on one interpreter is not.

    Returns
    -------
    object
        Whatever that call returns.
    """
    try:
        return read(*args, **kwargs)
    except OSError as error:
        raise ScanError(f"{path} could not be read: {error}") from error


def listing(directory: Path) -> typ.Callable[[], list[Path]]:
    """Defer a whole directory listing so a raise lands inside a guard.

    ``Path.iterdir`` starts its ``os.scandir`` at a different moment
    depending on the interpreter: lazily from a generator body on 3.12,
    eagerly on 3.14. Enumerating *and* materializing in one deferred call
    puts both behind :func:`scan`'s guard on either version. Pass the
    result to ``scan(directory, ...)``.

    Parameters
    ----------
    directory : Path
        The directory to enumerate when the returned callable is called.

    Returns
    -------
    callable
        A zero-argument callable returning the sorted listing.
    """

    def enumerate_once() -> list[Path]:
        """Enumerate ``directory``, raising inside the caller's guard."""
        return sorted(directory.iterdir())

    return enumerate_once


def _manifest_tables(manifest: str) -> list[dict[str, object]]:
    """Return each ``[[test]]`` table a manifest declares, in order."""
    declared = tomllib.loads(manifest).get("test", [])
    # ``[[test]]`` yields a list of tables. A value of any other type was
    # not declared in the form cargo accepts -- a singular ``[test]``
    # table, say -- so it names no target to read.
    if not isinstance(declared, list):
        return []
    return [dict(entry) for entry in declared if isinstance(entry, dict)]


def declared_test_targets(manifest: str) -> dict[str, str]:
    """Return each ``[[test]]`` target's name, keyed by its source path.

    The manifest is parsed as TOML rather than matched as text. Text
    patterns miss a declared target in silence: they accept only the
    double-quoted spelling of a value, so a single-quoted ``path`` or
    ``name`` reads as absent, ``[[test]]`` whitespace is not tolerated,
    and a block whose keys sit in another block was paired with that
    block's, because a regular expression cannot see where a table ends.
    A declared binary that goes unread is a binary the coverage
    assertion never asks for, which is the false negative the class
    exists to catch.

    ``path`` is optional to cargo, so a table without one is read as the
    source cargo defaults it to, ``tests/{name}.rs``. A manifest this
    reader cannot parse, and a table that declares no usable ``name``,
    yield no entry rather than raising: the caller asserts that a
    declared binary is asked for, and a manifest at odds with itself
    should leave that assertion reporting the binary, not the reader
    failing several frames from the cause.

    Parameters
    ----------
    manifest : str
        A crate's ``Cargo.toml`` text.

    Returns
    -------
    dict of str to str
        The declared binary name for each declared source path, the
        path relative to the crate directory.
    """
    try:
        tables = _manifest_tables(manifest)
    except tomllib.TOMLDecodeError:
        return {}
    declared: dict[str, str] = {}
    for table in tables:
        name = table.get("name")
        if not isinstance(name, str) or not name:
            continue
        path = table.get("path", f"tests/{name}.rs")
        if isinstance(path, str):
            declared[path] = name
    return declared
