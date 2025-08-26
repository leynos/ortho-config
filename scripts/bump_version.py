#!/usr/bin/env -S uv run
# /// script
# dependencies = ["tomlkit"]
# ///
"""Synchronise workspace and crate versions.

This tool updates the top-level workspace version and each member crate's
version to the supplied value. It exists to reduce the risk of publishing
mismatched versions across the workspace.

Examples
--------
Run with the desired semantic version:
    ./scripts/bump_version.py 1.2.3
"""
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

import tomlkit
from tomlkit.exceptions import TOMLKitError

def _set_version(
    toml_path: Path, version: str, dependency: str | None = None
) -> None:
    """Set version fields in a ``Cargo.toml`` file.

    The update respects existing formatting and comments by using ``tomlkit``.
    The optional ``dependency`` parameter allows synchronising an internal
    dependency's version alongside the package version.

    Examples
    --------
    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.NamedTemporaryFile('w+', suffix='.toml') as fh:
    ...     _ = fh.write('[package]\nname = "demo"\nversion = "0.1.0"')
    ...     fh.flush()
    ...     _set_version(Path(fh.name), '1.2.3')
    ...     fh.seek(0)
    ...     'version = "1.2.3"' in fh.read()
    True
    """
    with toml_path.open("r", encoding="utf-8") as fh:
        doc = tomlkit.parse(fh.read())

    if "workspace" in doc and "package" in doc["workspace"]:
        doc["workspace"]["package"]["version"] = version
    elif "package" in doc:
        doc["package"]["version"] = version

    if dependency:
        deps = doc.get("dependencies")
        if deps and dependency in deps:
            entry = deps[dependency]
            if isinstance(entry, dict):
                entry["version"] = version
            else:
                deps[dependency] = version

    text = tomlkit.dumps(doc)
    temp_dir = toml_path.parent
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=temp_dir, delete=False
    ) as tf:
        tf.write(text)
        temp_name = tf.name
    os.replace(temp_name, toml_path)


def _validate_args_and_setup(argv: list[str]) -> tuple[str, Path] | None:
    """Validate CLI arguments and resolve the workspace root.

    Parameters
    ----------
    argv
        Raw command-line arguments.

    Returns
    -------
    tuple[str, Path] | None
        The requested version and repository root if arguments are valid;
        ``None`` otherwise.

    Examples
    --------
    >>> _validate_args_and_setup(["bump_version.py", "1.2.3"])  # doctest: +ELLIPSIS
    ('1.2.3', Path(...))
    """
    if len(argv) != 2:
        prog = Path(argv[0]).name
        print(f"Usage: {prog} <version>", file=sys.stderr)
        return None
    version = argv[1]
    root = Path(__file__).resolve().parent.parent
    return version, root


def _resolve_member_paths(root: Path, members: list[str]) -> list[Path]:
    """Expand workspace member patterns to concrete paths.

    Parameters
    ----------
    root
        Workspace root directory.
    members
        Glob patterns for workspace members.

    Returns
    -------
    list[Path]
        Paths matched by the supplied patterns. Warnings are emitted for
        patterns that match nothing.

    Examples
    --------
    >>> _resolve_member_paths(Path('.'), ['scripts'])  # doctest: +ELLIPSIS
    [Path('scripts')]
    """
    paths: list[Path] = []
    for pattern in members:
        matches = list(root.glob(pattern))
        if not matches:
            print(
                f"Warning: No members matched pattern '{pattern}'",
                file=sys.stderr,
            )
            continue
        paths.extend(matches)
    return paths


def _update_member_version(member_path: Path, version: str) -> bool:
    """Update a member Cargo.toml with the supplied version.

    Parameters
    ----------
    member_path
        Path to the member's ``Cargo.toml`` file.
    version
        Semantic version to apply.

    Returns
    -------
    bool
        ``True`` if an error occurred while updating; ``False`` otherwise.

    Examples
    --------
    >>> _update_member_version(Path('Cargo.toml'), '1.2.3')
    False
    """
    try:
        dependency = (
            "ortho_config_macros" if member_path.parent.name == "ortho_config" else None
        )
        _set_version(member_path, version, dependency)
    except (
        TOMLKitError,
        OSError,
        TypeError,
        ValueError,
    ) as exc:  # pragma: no cover - defensive
        print(
            f"Error: Failed to set version for {member_path}: {exc}",
            file=sys.stderr,
        )
        return True
    return False


def _process_single_member(member_root: Path, version: str) -> bool:
    """Process a single workspace member.

    Parameters
    ----------
    member_root
        Directory or file matched from the workspace ``members`` patterns.
    version
        Semantic version to apply.

    Returns
    -------
    bool
        ``True`` if updating the member failed; ``False`` otherwise.

    Examples
    --------
    >>> _process_single_member(Path('scripts'), '1.2.3')
    False
    """
    member_path = (
        member_root / "Cargo.toml"
        if member_root.is_dir() or member_root.name != "Cargo.toml"
        else member_root
    )
    if not member_path.exists():
        print(
            f"Warning: Skipping missing member Cargo.toml at {member_path}",
            file=sys.stderr,
        )
        return False
    return _update_member_version(member_path, version)


def _process_members(root: Path, members: list[str], version: str) -> bool:
    """Update all workspace members to the supplied version.

    Parameters
    ----------
    root
        Workspace root directory.
    members
        Glob patterns for workspace members.
    version
        Semantic version to apply.

    Returns
    -------
    bool
        ``True`` if any member failed to update; ``False`` otherwise.

    Examples
    --------
    >>> _process_members(Path('.'), ['scripts'], '1.2.3')
    False
    """
    had_error = False
    for member_root in _resolve_member_paths(root, members):
        if _process_single_member(member_root, version):
            had_error = True
    return had_error


def main(argv: list[str]) -> int:
    """
    Update the workspace and member crate versions to the supplied value.

    Parameters
    ----------
    argv
        Command-line arguments where `argv[1]` is the target semantic version
        (for example, "1.2.3").

    Returns
    -------
    int
        Zero on success; non-zero if any member update fails or arguments are
        invalid.

    Examples
    --------
    >>> import sys
    >>> sys.exit(main(["bump_version.py", "1.2.3"]))
    """
    result = _validate_args_and_setup(argv)
    if result is None:
        return 1
    version, root = result
    workspace = root / "Cargo.toml"
    try:
        with workspace.open("r", encoding="utf-8") as fh:
            data = tomlkit.parse(fh.read())
    except TOMLKitError as exc:  # pragma: no cover - defensive
        print(f"Error: Failed to parse {workspace}: {exc}", file=sys.stderr)
        return 1
    members = data.get("workspace", {}).get("members", [])
    _set_version(workspace, version)
    had_error = _process_members(root, members, version)
    return 0 if not had_error else 1

if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main(sys.argv))
