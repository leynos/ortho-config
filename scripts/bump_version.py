#!/usr/bin/env -S uv run
# /// script
# dependencies = ["tomli-w"]
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

import tomllib
import tomli_w

def _set_version(toml_path: Path, version: str) -> None:
    """Set the package or workspace version in a Cargo.toml file."""
    with toml_path.open("rb") as fh:
        data = tomllib.load(fh)
    if "workspace" in data and "package" in data["workspace"]:
        data["workspace"]["package"]["version"] = version
    elif "package" in data:
        data["package"]["version"] = version

    text = tomli_w.dumps(data)
    temp_dir = toml_path.parent
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=temp_dir, delete=False
    ) as tf:
        tf.write(text)
        temp_name = tf.name
    os.replace(temp_name, toml_path)


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
    if len(argv) != 2:
        prog = Path(argv[0]).name
        print(f"Usage: {prog} <version>", file=sys.stderr)
        return 1
    version = argv[1]
    root = Path(__file__).resolve().parent.parent
    workspace = root / "Cargo.toml"
    try:
        with workspace.open("rb") as fh:
            data = tomllib.load(fh)
    except tomllib.TOMLDecodeError as exc:  # pragma: no cover - defensive
        print(f"Error: Failed to parse {workspace}: {exc}", file=sys.stderr)
        return 1
    members = data.get("workspace", {}).get("members", [])
    _set_version(workspace, version)
    had_error = False
    for pattern in members:
        matches = list(root.glob(pattern))
        if not matches:
            print(
                f"Warning: No members matched pattern '{pattern}'",
                file=sys.stderr,
            )
            continue
        for member_root in matches:
            member_path = (
                member_root / "Cargo.toml"
                if member_root.is_dir()
                else member_root
            )
            if member_path.name != "Cargo.toml":
                member_path = member_root / "Cargo.toml"
            if not member_path.exists():
                print(
                    f"Warning: Skipping missing member Cargo.toml at {member_path}",
                    file=sys.stderr,
                )
                continue
            try:
                _set_version(member_path, version)
            except (
                tomllib.TOMLDecodeError,
                OSError,
                TypeError,
                ValueError,
            ) as exc:  # pragma: no cover - defensive
                had_error = True
                print(
                    f"Error: Failed to set version for {member_path}: {exc}",
                    file=sys.stderr,
                )
    return 0 if not had_error else 1

if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main(sys.argv))
