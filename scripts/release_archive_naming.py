"""Naming and `cargo-binstall` template helpers for release archives.

`cargo-binstall` resolves a release asset by rendering the `pkg-url` and
`bin-dir` templates declared in `[package.metadata.binstall]`. The archive the
release workflow uploads therefore has to be named, and laid out internally,
exactly as those templates render. This module owns both sides of that
agreement so the packaging script and its contract tests derive the names from
one place instead of restating the convention.

Examples
--------
Derive the archive stem and member path for a Windows target:

>>> archive_stem("cargo-orthohelp", "x86_64-pc-windows-msvc", "0.9.0")
'cargo-orthohelp-x86_64-pc-windows-msvc-v0.9.0'
>>> binary_extension("x86_64-pc-windows-msvc")
'.exe'
"""

from __future__ import annotations

import re
import tomllib
from pathlib import Path

__all__ = [
    "ARCHIVE_SUFFIX",
    "ManifestError",
    "archive_file_name",
    "archive_member_path",
    "archive_stem",
    "binary_extension",
    "binstall_metadata",
    "manifest_version",
    "render_binstall_template",
]

#: Suffix cargo-binstall renders for `{ archive-suffix }` when `pkg-fmt` is
#: `tgz`. It probes both spellings the format accepts, `.tgz` and `.tar.gz`,
#: so either would be downloadable; `.tgz` is the one this repository
#: publishes and the one the auditor and contract tests expect.
ARCHIVE_SUFFIX = ".tgz"

#: Matches a `leon` placeholder as cargo-binstall writes it, tolerating the
#: optional padding spaces used throughout the estate's manifests.
_PLACEHOLDER_RE = re.compile(r"\{\s*([A-Za-z0-9_-]+)\s*\}")

_WINDOWS_TARGET_MARKER = "-windows-"


class ManifestError(RuntimeError):
    """Raised when a Cargo manifest lacks the data the packager needs."""


def _load_manifest(manifest_path: Path) -> dict[str, object]:
    """Return the parsed TOML document at *manifest_path*.

    Raises
    ------
    ManifestError
        Raised when the manifest is missing or is not valid TOML.
    """
    try:
        raw = manifest_path.read_bytes()
    except OSError as err:
        message = f"cannot read Cargo manifest {manifest_path}: {err}"
        raise ManifestError(message) from err
    try:
        return tomllib.loads(raw.decode("utf-8"))
    except (tomllib.TOMLDecodeError, UnicodeDecodeError) as err:
        message = f"cannot parse Cargo manifest {manifest_path}: {err}"
        raise ManifestError(message) from err


def _workspace_version(manifest_path: Path) -> str:
    """Return `workspace.package.version` from the nearest workspace manifest.

    Raises
    ------
    ManifestError
        Raised when no ancestor manifest declares a workspace package version.
    """
    for ancestor in manifest_path.resolve().parents:
        candidate = ancestor / "Cargo.toml"
        if candidate == manifest_path.resolve() or not candidate.is_file():
            continue
        document = _load_manifest(candidate)
        workspace = document.get("workspace")
        if not isinstance(workspace, dict):
            continue
        package = workspace.get("package")
        if isinstance(package, dict) and isinstance(package.get("version"), str):
            return package["version"]
    message = (
        f"{manifest_path} inherits package.version from the workspace, but no "
        "ancestor Cargo.toml declares workspace.package.version"
    )
    raise ManifestError(message)


def manifest_version(manifest_path: Path) -> str:
    """Return the package version declared by *manifest_path*.

    Workspace inheritance (`version.workspace = true`) is resolved against the
    nearest ancestor manifest that declares `workspace.package.version`.

    Raises
    ------
    ManifestError
        Raised when the manifest declares no resolvable package version.

    Examples
    --------
    >>> manifest_version(Path("cargo-orthohelp/Cargo.toml"))  # doctest: +SKIP
    '0.9.0'
    """
    document = _load_manifest(manifest_path)
    package = document.get("package")
    if not isinstance(package, dict):
        message = f"{manifest_path} declares no [package] table"
        raise ManifestError(message)
    version = package.get("version")
    if isinstance(version, str):
        return version
    if isinstance(version, dict) and version.get("workspace") is True:
        return _workspace_version(manifest_path)
    message = f"{manifest_path} declares no usable package.version"
    raise ManifestError(message)


def binstall_metadata(manifest_path: Path) -> dict[str, str]:
    """Return the `[package.metadata.binstall]` table from *manifest_path*.

    Raises
    ------
    ManifestError
        Raised when the table is absent or holds a non-string entry.

    Examples
    --------
    >>> sorted(binstall_metadata(Path("cargo-orthohelp/Cargo.toml")))  # doctest: +SKIP
    ['bin-dir', 'pkg-fmt', 'pkg-url']
    """
    document = _load_manifest(manifest_path)
    package = document.get("package")
    metadata = package.get("metadata") if isinstance(package, dict) else None
    binstall = metadata.get("binstall") if isinstance(metadata, dict) else None
    if not isinstance(binstall, dict):
        message = f"{manifest_path} declares no [package.metadata.binstall] table"
        raise ManifestError(message)
    non_strings = sorted(key for key, value in binstall.items() if not isinstance(value, str))
    if non_strings:
        message = (
            f"{manifest_path} [package.metadata.binstall] entries must be strings; "
            f"offending keys: {', '.join(non_strings)}"
        )
        raise ManifestError(message)
    return dict(binstall)


def binary_extension(target: str) -> str:
    """Return the executable suffix cargo produces for *target*.

    Examples
    --------
    >>> binary_extension("aarch64-apple-darwin")
    ''
    >>> binary_extension("x86_64-pc-windows-msvc")
    '.exe'
    """
    return ".exe" if _WINDOWS_TARGET_MARKER in target else ""


def archive_stem(name: str, target: str, version: str) -> str:
    """Return the archive stem shared by the file name and its root directory.

    Examples
    --------
    >>> archive_stem("cargo-orthohelp", "x86_64-unknown-linux-gnu", "0.9.0")
    'cargo-orthohelp-x86_64-unknown-linux-gnu-v0.9.0'
    """
    return f"{name}-{target}-v{version}"


def archive_file_name(name: str, target: str, version: str) -> str:
    """Return the release asset file name for *target*.

    Examples
    --------
    >>> archive_file_name("cargo-orthohelp", "aarch64-apple-darwin", "0.9.0")
    'cargo-orthohelp-aarch64-apple-darwin-v0.9.0.tgz'
    """
    return f"{archive_stem(name, target, version)}{ARCHIVE_SUFFIX}"


def archive_member_path(name: str, target: str, version: str, binary: str) -> str:
    """Return the in-archive path of *binary*, as `bin-dir` must render it.

    Examples
    --------
    >>> archive_member_path(
    ...     "cargo-orthohelp", "x86_64-pc-windows-msvc", "0.9.0", "cargo-orthohelp"
    ... )
    'cargo-orthohelp-x86_64-pc-windows-msvc-v0.9.0/cargo-orthohelp.exe'
    """
    stem = archive_stem(name, target, version)
    return f"{stem}/{binary}{binary_extension(target)}"


def render_binstall_template(template: str, fields: dict[str, str]) -> str:
    """Substitute cargo-binstall `{ key }` placeholders in *template*.

    Raises
    ------
    KeyError
        Raised when *template* references a placeholder absent from *fields*,
        which would leave the rendered value silently wrong.

    Examples
    --------
    >>> render_binstall_template("{ name }-v{ version }", {"name": "x", "version": "1"})
    'x-v1'
    """

    def substitute(match: re.Match[str]) -> str:
        key = match.group(1)
        if key not in fields:
            message = f"template placeholder {{ {key} }} has no value"
            raise KeyError(message)
        return fields[key]

    return _PLACEHOLDER_RE.sub(substitute, template)
