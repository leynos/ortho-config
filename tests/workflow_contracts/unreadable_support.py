"""Denying access to a path so a refusal can be observed.

The workflow contracts exist partly to refuse a source they cannot read rather
than report it as absent, and that difference is invisible in a tree where
every file is readable: the two readings agree everywhere the gate looks. A
test can only tell them apart by making a read fail, which is what
:func:`denied` does.

A mode denies a read only to a non-root process, and only on a platform whose
``os.chmod`` changes more than the read-only flag, so
:data:`DENIED_ACCESS_IS_UNOBSERVABLE` names the two situations in which the
refusal is simply absent. Tests that use :func:`denied` skip in those and run
everywhere else; the Ubuntu leg of CI is where they are exercised.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import contextlib
import os
import stat
import sys
import tempfile
import typing as typ
from pathlib import Path

__all__ = ["DENIED_ACCESS_IS_UNOBSERVABLE", "denied"]

#: Whether a denied mode is unobservable in this process.
#:
#: A directory whose mode denies a listing or a search denies neither to root,
#: so the refusal cannot be seen when the suite runs as root. Nor can it be
#: seen on Windows, where ``os.chmod`` changes only the read-only flag: no mode
#: makes a file unopenable there, so a test asserting the refusal would fail
#: for a reason that says nothing about the reading.
DENIED_ACCESS_IS_UNOBSERVABLE: typ.Final[bool] = (
    hasattr(os, "geteuid") and os.geteuid() == 0
) or sys.platform == "win32"


@contextlib.contextmanager
def denied(path: Path) -> typ.Iterator[None]:
    """Deny every access to ``path`` until the block exits.

    A directory loses its permission bits, so a path beneath it can be neither
    searched nor listed; a file loses them, so opening it fails. The original
    mode is restored in a ``finally`` rather than after the block, because an
    assertion that fails inside the block must not leave pytest a temporary
    tree it cannot delete.

    That the denial itself happens is asserted by the tests that use this,
    which skip where a mode denies nothing; the example below pins the half
    that holds on every platform, which is the restore.

    Parameters
    ----------
    path : pathlib.Path
        The file or directory to deny access to for the duration of the block.

    Yields
    ------
    None
        Control returns to the caller with ``path``'s permission bits cleared.

    Examples
    --------
    >>> with tempfile.TemporaryDirectory() as scratch:
    ...     hidden = Path(scratch, "hidden")
    ...     hidden.mkdir()
    ...     before = stat.S_IMODE(hidden.stat().st_mode)
    ...     with denied(hidden):
    ...         pass
    ...     stat.S_IMODE(hidden.stat().st_mode) == before
    True
    """
    original = path.stat().st_mode
    os.chmod(path, 0o000)
    try:
        yield
    finally:
        os.chmod(path, original)