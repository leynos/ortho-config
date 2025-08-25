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
    """Update the workspace and member crate versions to the supplied value."""
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
    for member in members:
        member_path = root / member / "Cargo.toml"
        if not member_path.exists():
            print(
                f"Warning: Skipping missing member Cargo.toml at {member_path}",
                file=sys.stderr,
            )
            continue
        try:
            _set_version(member_path, version)
        except Exception as exc:  # pragma: no cover - defensive
            print(
                f"Error: Failed to set version for {member_path}: {exc}",
                file=sys.stderr,
            )
    return 0

if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main(sys.argv))
