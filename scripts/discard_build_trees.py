"""Discard named build trees under a Cargo target directory, reporting sizes.

The Linux ``build-test`` leg runs on ``ubicloud-standard-4``. By the time
coverage starts, the lint steps have filled ``target/`` with trees coverage
never reads: rustdoc and Clippy output under ``debug`` and ``doc``, and
Whitaker's under ``dylint``. ``ubicloud-standard-2`` was the failed sizing
trial: two runs on it died of a full disk during coverage. ``ubicloud-standard-4``
is the current runner, and the job still removes those trees first to keep
headroom.

Each tree is named, its size is printed, and a name with nothing on disk is
reported as such rather than silently skipped. A discard that removed nothing
while claiming success is how an earlier estate step hid a wrong path.

Examples
--------
Remove a tree and report what went:

>>> import tempfile
>>> root = Path(tempfile.mkdtemp())
>>> (root / "debug").mkdir()
>>> _ = (root / "debug" / "a").write_bytes(b"x" * 10)
>>> [(r.name, r.size) for r in discard(root, ["debug", "doc"])]
[('debug', 10), ('doc', None)]
>>> (root / "debug").exists()
False
"""

from __future__ import annotations

import contextlib
import dataclasses
import os
import shutil
import stat
import sys
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from collections.abc import Iterator

__all__ = ["DiscardError", "Removal", "discard", "main", "tree_size"]


class DiscardError(Exception):
    """A filesystem operation on a named tree failed.

    Wraps the underlying ``OSError`` as ``__cause__`` so the command can name
    the path and reason without a traceback.
    """


@dataclasses.dataclass(frozen=True, slots=True)
class Removal:
    """One named tree: its name and the bytes removed, or ``None`` if absent."""

    name: str
    size: int | None


@contextlib.contextmanager
def _filesystem(action: str, path: Path) -> Iterator[None]:
    """Report an ``OSError`` raised by ``action`` on ``path`` as ``DiscardError``."""
    try:
        yield
    except OSError as error:
        message = f"cannot {action} {path}: {error}"
        raise DiscardError(message) from error


def _raise_scan_error(error: OSError) -> None:
    """Re-raise a directory-scan error, which ``os.walk`` would otherwise drop."""
    raise error


def tree_size(path: Path) -> int:
    """Return the total size in bytes of the regular files under ``path``.

    Parameters
    ----------
    path : Path
        Directory to measure.

    Returns
    -------
    int
        The summed logical size of every regular file beneath ``path``.
        Symlinks are not followed, since ``rmtree`` removes the link and not
        its target.

    Raises
    ------
    DiscardError
        If a directory cannot be scanned or a file cannot be examined, for
        example on a permission failure or a file removed mid-walk.

    Examples
    --------
    >>> import tempfile
    >>> root = Path(tempfile.mkdtemp())
    >>> _ = (root / "f").write_bytes(b"abc")
    >>> tree_size(root)
    3
    """
    with _filesystem("measure", path):
        sizes = (
            (Path(directory) / name).lstat()
            for directory, _, names in os.walk(path, onerror=_raise_scan_error)
            for name in names
        )
        return sum(status.st_size for status in sizes if stat.S_ISREG(status.st_mode))


def discard(root: Path, names: list[str]) -> list[Removal]:
    """Remove each named tree under ``root`` and report what was removed.

    A name is accepted only when it is one relative path component naming a
    direct child of ``root``: a nested or absolute name, one that escapes
    ``root``, or one that reaches its tree through a symlink or a ``..``
    component, is refused rather than followed. That keeps a link such
    as ``debug -> llvm-cov-target`` from deleting the tree it points at under
    another name. The validated named path is removed, never its resolved
    target.

    Parameters
    ----------
    root : Path
        The Cargo target directory; nothing outside it is ever deleted.
    names : list[str]
        Tree names relative to ``root``, for example ``["debug", "doc"]``.

    Returns
    -------
    list[Removal]
        One entry per name, in order; ``size`` is ``None`` for an absent tree.

    Raises
    ------
    ValueError
        If a name is not a single plain component directly under ``root``.
    DiscardError
        If ``root`` is not a directory, or a path cannot be resolved,
        examined, measured or removed. Trees handled before the failure stay
        removed; none after it are touched.
    """
    base = _target_directory(root)
    return [_discard_one(base, name) for name in names]


def _target_directory(root: Path) -> Path:
    """Return the resolved ``root``, refusing one that is not a directory.

    A wrong target path would otherwise report every tree as absent and let
    the step pass without freeing anything.
    """
    with _filesystem("resolve", root):
        base = root.resolve()
        if not base.is_dir():
            message = f"cannot discard under {base}: not a directory"
            raise DiscardError(message)
    return base


def _plain_tree_path(base: Path, name: str) -> Path:
    """Return ``base / name`` once it is proven a plain path under ``base``.

    Raises
    ------
    ValueError
        If the name is the target itself, escapes it, is absolute or nested,
        or reaches its tree through a symlink or ``..`` component.

    DiscardError
        If the path cannot be resolved.
    """
    tree = base / name
    with _filesystem("resolve", tree):
        resolved = tree.resolve()
    if resolved == base:
        message = f"{name!r} names the target directory {base} itself"
        raise ValueError(message)
    if not resolved.is_relative_to(base):
        message = f"{name!r} resolves outside {base}"
        raise ValueError(message)
    if Path(name).is_absolute() or len(Path(name).parts) != 1:
        message = f"{name!r} is not a single relative name directly under {base}"
        raise ValueError(message)
    if resolved != tree:
        message = f"{name!r} is an alias for {resolved}, not a plain path under {base}"
        raise ValueError(message)
    return tree


def _discard_one(base: Path, name: str) -> Removal:
    """Remove the tree ``name`` under ``base`` and report its size, or ``None``."""
    tree = _plain_tree_path(base, name)
    if not _is_present(tree):
        return Removal(name, None)
    size = tree_size(tree)
    with _filesystem("remove", tree):
        shutil.rmtree(tree)
    return Removal(name, size)


def _is_present(tree: Path) -> bool:
    """Return whether ``tree`` exists, without hiding an environmental failure.

    ``Path.exists`` can report a permission or I/O failure as absence, which
    would let a cleanup that freed nothing pass; only a missing path is absent.
    """
    try:
        tree.lstat()
    except FileNotFoundError:
        return False
    except OSError as error:
        message = f"cannot examine {tree}: {error}"
        raise DiscardError(message) from error
    return True


def main(argv: list[str]) -> int:
    """Discard ``argv[1:]`` under the target directory ``argv[0]``.

    A refused name or a failed filesystem operation is reported on standard
    error with a non-zero status, so the workflow step fails with a clear
    diagnostic instead of a traceback.

    Parameters
    ----------
    argv : list[str]
        The target directory followed by the tree names to discard.

    Returns
    -------
    int
        ``0`` when every name was handled, ``1`` when a name was refused or a
        filesystem operation failed, ``2`` when no target directory was given.
    """
    if not argv:
        print("usage: discard_build_trees.py TARGET_DIR [TREE ...]", file=sys.stderr)
        return 2
    root, *names = argv
    try:
        removals = discard(Path(root), names)
    except (DiscardError, ValueError) as error:
        print(f"discard failed: {error}", file=sys.stderr)
        return 1
    for removal in removals:
        if removal.size is None:
            print(f"{removal.name}: nothing to remove")
        else:
            print(f"{removal.name}: removed {removal.size / 2**30:.2f} GiB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
