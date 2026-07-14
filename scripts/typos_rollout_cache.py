"""Provide cache support types and atomic writes for the spelling helper."""

from __future__ import annotations

import dataclasses as dc
import pathlib
import tempfile
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc


@dc.dataclass(frozen=True)
class RefreshResult:
    """Describe whether the untracked shared dictionary cache changed.

    Attributes
    ----------
    status
        Refresh outcome such as ``refreshed``, ``current`` or ``stale-cache``.
    cache
        Path to the validated untracked dictionary cache.
    """

    status: str
    cache: pathlib.Path


@dc.dataclass(frozen=True)
class CacheTargets:
    """Group dictionary cache persistence paths.

    Attributes
    ----------
    cache
        Untracked dictionary cache destination.
    metadata
        Sidecar containing source identity and freshness validators.
    """

    cache: pathlib.Path
    metadata: pathlib.Path


class RemoteResponse(typ.Protocol):
    """Expose the HTTP response surface used by cache refresh.

    Attributes
    ----------
    status
        HTTP response status code.
    headers
        Response headers used for freshness and metadata persistence.
    """

    status: int
    headers: cabc.Mapping[str, str]

    def read(self) -> bytes:
        """Read the response body.

        Returns
        -------
        bytes
            Complete response body.

        Raises
        ------
        OSError
            If the response body cannot be read.
        """
        ...


def atomic_write(path: pathlib.Path, content: bytes) -> None:
    """Write content beside a path and atomically replace the destination.

    Parameters
    ----------
    path
        Destination path whose parent directory is created when absent.
    content
        Bytes to persist.

    Raises
    ------
    OSError
        If directory creation, writing, closing or replacement fails.
    """
    path.parent.mkdir(parents=True, exist_ok=True)
    stream = tempfile.NamedTemporaryFile(
        delete=False, dir=path.parent, prefix=f".{path.name}."
    )
    temporary = pathlib.Path(stream.name)
    try:
        with stream:
            stream.write(content)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)
