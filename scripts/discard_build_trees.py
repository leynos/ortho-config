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

import dataclasses
import shutil
import sys
from pathlib import Path

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

    Raises
    ------
    DiscardError
        If a directory cannot be traversed or a file cannot be examined, for
        example on a permission failure or a file removed mid-walk.

    Examples
    --------
    >>> import tempfile
    >>> root = Path(tempfile.mkdtemp())
    >>> _ = (root / "f").write_bytes(b"abc")
    >>> tree_size(root)
    3
    """
    try:
        return sum(entry.stat().st_size for entry in path.rglob("*") if entry.is_file())
    except OSError as error:
        message = f"cannot measure {path}: {error}"
        raise DiscardError(message) from error


def discard(root: Path, names: list[str]) -> list[Removal]:
    """Remove each named tree under ``root`` and report what was removed.

    A name is accepted only when it is a plain path directly under ``root``:
    a name that escapes ``root``, or that reaches its tree through a symlink or
    a ``..`` component, is refused rather than followed. That keeps a link such
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
        If a name resolves outside ``root`` or is not a plain path under it.
    DiscardError
        If a tree cannot be measured or removed. Trees handled before the
        failure stay removed; none after it are touched.
    """
    base = root.resolve()
    removals = []
    for name in names:
        tree = base / name
        resolved = tree.resolve()
        if base not in resolved.parents:
            message = f"{name!r} resolves outside {base}"
            raise ValueError(message)
        if resolved != tree:
            message = f"{name!r} is an alias for {resolved}, not a plain path under {base}"
            raise ValueError(message)
        if not tree.exists():
            removals.append(Removal(name, None))
            continue
        size = tree_size(tree)
        try:
            shutil.rmtree(tree)
        except OSError as error:
            message = f"cannot remove {tree}: {error}"
            raise DiscardError(message) from error
        removals.append(Removal(name, size))
    return removals


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
        filesystem operation failed.
    """
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
