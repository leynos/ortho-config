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


class ManifestError(ValueError):
    """Raised when a manifest this module reads is not a manifest.

    A read that succeeded and a parse that failed are different faults,
    so this is not a :class:`ScanError`: the bytes arrived, they are
    simply not TOML. Kept separate for the same reason ``nextest_errors``
    keeps its configuration fault apart from its budget faults -- the
    caller that must report a manifest at odds with itself should not
    have to catch a filesystem error to do it.

    Both the message and the manifest's path name the file, so a failure
    points at the source rather than at the frame that happened to call
    this. A parse failure keeps the original ``TOMLDecodeError`` as
    ``__cause__``; a table declared in a shape cargo does not accept has
    no earlier exception to chain, so it names the offending value
    instead.
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


def exists(path: Path) -> typ.Callable[[], bool]:
    """Defer a presence check so a failure to tell lands inside a guard.

    ``Path.is_file`` and ``Path.is_dir`` are **not** usable as absence
    tests here. Since 3.14 they answer ``False`` for any ``OSError`` the
    operating system raises, not only for a path that is missing: on a
    directory this process cannot search, ``is_file`` returns ``False``
    instead of raising, where 3.12 raised ``PermissionError``. The gate
    runs 3.14, so a source behind an unsearchable directory read as
    absent and left the class silently -- the precise false negative this
    module exists to refuse.

    Only ``FileNotFoundError`` is an answer. Every other failure
    propagates as a :class:`ScanError` naming ``path``. Pass the result
    to :func:`scan` -- ``scan(path, exists(path))`` -- rather than a
    bound ``Path`` predicate, for the same reason :func:`listing` is a
    factory: the deferred call puts the check behind ``scan``'s guard.

    Parameters
    ----------
    path : Path
        The path to check for presence when the callable is called.

    Returns
    -------
    callable
        A zero-argument callable returning whether that path is there.
    """

    def present() -> bool:
        """Answer whether ``path`` is there, raising on a check that cannot tell."""
        try:
            path.stat()
        except FileNotFoundError:
            return False
        return True

    return present


#: A read's own parameter list, so :func:`scan` forwards it rather than
#: erasing it to ``Any``. The reads here return three different types --
#: ``bool`` from :func:`exists`, ``str`` from ``Path.read_text``,
#: ``list[Path]`` from :func:`listing` -- and a caller that receives ``Any``
#: gives up the contract at every call site, which is exactly where a wrong
#: type goes unnoticed.
_P = typ.ParamSpec("_P")

#: What the forwarded read returns, preserved as ``scan``'s own result.
_R = typ.TypeVar("_R")


def scan(
    path: Path,
    read: typ.Callable[_P, _R],
    *args: _P.args,
    **kwargs: _P.kwargs,
) -> _R:
    """Call one ``Path`` read, refusing a failure that names no path.

    Every filesystem read here goes through this: ``path`` is the path
    the call is over and the one a failure names, and ``read`` is the
    call to make -- a bound ``Path`` method such as ``Path.read_text``,
    or a deferred callable such as :func:`listing`. A path that is merely
    absent is an answer, so :func:`exists` answers ``False`` for it
    rather than raising; any other failure is a :class:`ScanError` naming
    ``path``, the original as ``__cause__``. Do not pass a bound
    ``Path.is_file`` or ``Path.is_dir`` here: since 3.14 those swallow
    every ``OSError``, which turns an inaccessible path into a silent
    absence. See :func:`exists`.

    The callable form is not a convenience. ``read(*args)`` evaluates its
    arguments *before* the ``try`` below, and ``Path.iterdir`` starts its
    ``os.scandir`` at a different moment depending on the interpreter:
    lazily from a generator body on 3.12, eagerly on 3.14. Passing
    ``list`` and ``tests.iterdir()`` therefore guarded the read on one
    interpreter and left the raise uncaught on the other. Deferring the
    whole read -- enumerate and materialize together, as :func:`listing`
    does -- is version-independent; testing it on one interpreter is not.

    Parameters
    ----------
    path : Path
        The path the read is over, and the one a failure names. A merely
        absent path is forwarded to ``read`` rather than judged here, so
        the choice of whether absence is an answer stays with the caller.
    read : callable
        The read to make: a bound ``Path`` method such as
        ``Path.read_text``, or a deferred callable such as
        :func:`listing` or :func:`exists`. Its remaining parameters are
        declared by ``*args`` and ``**kwargs`` below.
    *args : object
        Positional arguments forwarded to ``read`` when it is called.
    **kwargs : object
        Keyword arguments forwarded to ``read`` when it is called.

    Returns
    -------
    _R
        Whatever that call returns, at its own type rather than ``Any``.

    Raises
    ------
    ScanError
        If the read raises ``OSError`` other than ``FileNotFoundError``,
        naming ``path`` and keeping the original as ``__cause__``.
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


def _manifest_tables(manifest: str, at: Path) -> list[dict[str, object]]:
    """Return each ``[[test]]`` table a manifest declares, in order.

    ``at`` names the manifest in the message a refusal carries, so a
    failure points at the file rather than at whichever frame called
    this. It is likewise what a :class:`ManifestError` is built from.

    Parameters
    ----------
    manifest : str
        The manifest's text.
    at : Path
        The manifest's path, for naming it in a refusal.

    Raises
    ------
    ManifestError
        If the text is not TOML, or if its ``test`` key is present and
        not the list of tables cargo accepts for ``[[test]]``. A parse
        failure keeps the original ``TOMLDecodeError`` as ``__cause__``;
        a shape failure names the offending value in the message, there
        being no earlier exception to chain.
    """
    try:
        loaded = tomllib.loads(manifest)
    except tomllib.TOMLDecodeError as error:
        raise ManifestError(f"{at} is not valid TOML: {error}") from error
    declared = loaded.get("test", [])
    # ``[[test]]`` yields a list of tables. A value of any other type was
    # not declared in the form cargo accepts -- a singular ``[test]``
    # table, say -- so it names no target to read, and reading on would
    # hand the caller an inventory missing every target the manifest
    # meant to declare.
    if not isinstance(declared, list):
        raise ManifestError(
            f"{at} declares `test` as {declared!r}, not the list of "
            f"tables `[[test]]` yields"
        )
    return [dict(entry) for entry in declared if isinstance(entry, dict)]


def declared_test_targets(manifest: str, at: Path) -> dict[str, str]:
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
    source cargo defaults it to, ``tests/{name}.rs``. A table that
    declares no usable ``name`` yields no entry rather than raising: it
    names no target to enumerate, so there is nothing for the caller to
    ask about, and refusing the whole manifest over one unusable row
    would hide the targets that *are* readable. A manifest that is not
    TOML, or whose ``test`` key is not ``[[test]]``'s list of tables, is
    a different case and is **refused** rather than read as empty. An
    empty mapping is indistinguishable from "this crate declares no
    targets", which is exactly the inventory the coverage assertion
    cannot see a gap in.

    Parameters
    ----------
    manifest : str
        A crate's ``Cargo.toml`` text.
    at : Path
        The manifest's path, used to name it in a refusal. Required, so
        a refusal can always name the file rather than a placeholder.

    Returns
    -------
    dict of str to str
        The declared binary name for each declared source path, the
        path relative to the crate directory.

    Raises
    ------
    ManifestError
        If the text is not TOML, or if its ``test`` key is not the list
        of tables ``[[test]]`` yields. Propagated from
        :func:`_manifest_tables`, which is where the failure is
        diagnosed.
    """
    tables = _manifest_tables(manifest, at)
    declared: dict[str, str] = {}
    for table in tables:
        name = table.get("name")
        if not isinstance(name, str) or not name:
            continue
        path = table.get("path", f"tests/{name}.rs")
        if isinstance(path, str):
            declared[path] = name
    return declared
