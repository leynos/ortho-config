#!/usr/bin/env uv
# /// script
# dependencies = ["toml-w"]
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

import sys
from pathlib import Path
import tomllib
import toml_w

def _set_version(toml_path: Path, version: str) -> None:
    """Set the package or workspace version in a Cargo.toml file."""
    with toml_path.open("rb") as fh:
        data = tomllib.load(fh)
    if "workspace" in data and "package" in data["workspace"]:
        data["workspace"]["package"]["version"] = version
    elif "package" in data:
        data["package"]["version"] = version
    toml_path.write_text(toml_w.dumps(data))

def main(argv: list[str]) -> int:
    if len(argv) != 2:
        prog = Path(argv[0]).name
        print(f"Usage: {prog} <version>")
        return 1
    version = argv[1]
    root = Path(__file__).resolve().parent.parent
    workspace = root / "Cargo.toml"
    with workspace.open("rb") as fh:
        data = tomllib.load(fh)
    members = data.get("workspace", {}).get("members", [])
    _set_version(workspace, version)
    for member in members:
        _set_version(root / member / "Cargo.toml", version)
    return 0

if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main(sys.argv))
