#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Audit staged or downloaded `cargo-orthohelp` release archives.

For every requested target the auditor checks that the archive exists under
the expected name, that its `.sha256` sidecar verifies, and that the archive
holds exactly the member the `bin-dir` template renders. Running it against a
draft release's assets catches a layout mistake before the release is
published, where cargo-binstall would otherwise report an empty source path.

Examples
--------
Audit every published target in `release-dist`:

```
uv run --script scripts/verify_release_archives.py --dist-dir release-dist
```
"""

from __future__ import annotations

import argparse
import hashlib
import sys
import tarfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from release_archive_naming import (  # noqa: E402
    ARCHIVE_SUFFIX,
    ManifestError,
    archive_file_name,
    binary_extension,
    binstall_metadata,
    manifest_version,
    render_binstall_template,
)

#: Targets the release workflow publishes. Kept here so the auditor, the
#: workflow matrix, and the contract tests can be compared against one list.
RELEASE_TARGETS = (
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
)

PACKAGE_NAME = "cargo-orthohelp"
DEFAULT_MANIFEST = Path("cargo-orthohelp") / "Cargo.toml"


def expected_member(metadata: dict[str, str], target: str, version: str) -> str:
    """Return the archive member path the `bin-dir` template renders.

    Examples
    --------
    >>> metadata = {"bin-dir": "{ name }-{ target }-v{ version }/{ bin }{ binary-ext }"}
    >>> expected_member(metadata, "x86_64-pc-windows-msvc", "0.9.0")
    'cargo-orthohelp-x86_64-pc-windows-msvc-v0.9.0/cargo-orthohelp.exe'
    """
    return render_binstall_template(
        metadata["bin-dir"],
        {
            "name": PACKAGE_NAME,
            "bin": PACKAGE_NAME,
            "target": target,
            "version": version,
            "binary-ext": binary_extension(target),
            "archive-suffix": ARCHIVE_SUFFIX,
        },
    )


def _verify_sidecar(archive_path: Path) -> str | None:
    """Return a failure description for *archive_path*'s sidecar, else `None`."""
    sidecar_path = archive_path.with_name(f"{archive_path.name}.sha256")
    if not sidecar_path.is_file():
        return f"{sidecar_path.name} is missing"
    recorded = sidecar_path.read_text(encoding="utf-8").split()
    if not recorded:
        return f"{sidecar_path.name} is empty"
    actual = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    if recorded[0] != actual:
        return f"{sidecar_path.name} records {recorded[0]}, archive hashes to {actual}"
    return None


def _verify_members(archive_path: Path, member: str) -> str | None:
    """Return a failure description for *archive_path*'s members, else `None`."""
    try:
        with tarfile.open(archive_path, mode="r:gz") as tar:
            members = [entry.name for entry in tar.getmembers() if entry.isfile()]
    except (tarfile.TarError, OSError) as err:
        return f"{archive_path.name} could not be read as a gzip tar archive: {err}"
    if members != [member]:
        return f"{archive_path.name} holds {members!r}, expected exactly [{member!r}]"
    return None


def audit_target(dist_dir: Path, metadata: dict[str, str], target: str, version: str) -> list[str]:
    """Return the audit failures for one *target*, empty when it passes."""
    archive_path = dist_dir / archive_file_name(PACKAGE_NAME, target, version)
    if not archive_path.is_file():
        return [f"{archive_path.name} is missing"]
    failures = [
        failure
        for failure in (
            _verify_sidecar(archive_path),
            _verify_members(archive_path, expected_member(metadata, target, version)),
        )
        if failure is not None
    ]
    return failures


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Parse the auditor's command-line arguments."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--dist-dir",
        type=Path,
        default=Path("dist"),
        help="Directory holding the archives and sidecars to audit.",
    )
    parser.add_argument(
        "--manifest-path",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="Path to the cargo-orthohelp Cargo.toml.",
    )
    parser.add_argument(
        "--release-version",
        default=None,
        help="Release version without the leading v; defaults to the manifest version.",
    )
    parser.add_argument(
        "--target",
        action="append",
        dest="targets",
        default=None,
        help="Target to audit; repeat to override the published target set.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    """Audit every requested target and report each failure.

    Raises
    ------
    SystemExit
        Raised with a non-empty message when any target fails the audit.

    Examples
    --------
    >>> main(["--dist-dir", "release-dist"])  # doctest: +SKIP
    """
    args = _parse_args(argv)
    try:
        metadata = binstall_metadata(args.manifest_path)
        version = args.release_version or manifest_version(args.manifest_path)
    except ManifestError as err:
        raise SystemExit(str(err)) from err

    failures: list[str] = []
    for target in args.targets or RELEASE_TARGETS:
        target_failures = audit_target(args.dist_dir, metadata, target, version)
        failures.extend(f"{target}: {failure}" for failure in target_failures)
        if not target_failures:
            print(f"{target}: archive and sidecar verified")

    if failures:
        raise SystemExit("\n".join(failures))


if __name__ == "__main__":
    main()
