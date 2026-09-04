#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Build and package `cargo-orthohelp` release archives for cargo-binstall.

The script builds the release binary for one Rust target, stages it under the
archive root that `[package.metadata.binstall]` expects, writes a reproducible
`.tgz`, and emits a `sha256sum`-compatible `.sha256` sidecar beside it. Standard
output carries exactly one line, the archive path, so callers can capture it.

Examples
--------
Package the Linux x86-64 target from a checkout:

```
uv run --script scripts/release_archive.py x86_64-unknown-linux-gnu
```
"""

from __future__ import annotations

import argparse
import gzip
import hashlib
import io
import os
import subprocess
import sys
import tarfile
from dataclasses import dataclass
from pathlib import Path

# The naming helpers are a sibling module rather than a package, so the
# script directory must join sys.path before the import can resolve; the
# import therefore cannot sit at the top of the file.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from release_archive_naming import (  # noqa: E402  # see the sys.path note above
    ManifestError,
    archive_file_name,
    archive_stem,
    binary_extension,
    manifest_version,
)

#: Crate whose binary is published. The package name doubles as the binstall
#: `{ name }` field and as the binary name inside the archive.
PACKAGE_NAME = "cargo-orthohelp"

#: Default manifest, relative to the repository root.
DEFAULT_MANIFEST = Path("cargo-orthohelp") / "Cargo.toml"

#: Fixed timestamp for archive members. Release archives must hash identically
#: when rebuilt from the same tag, so no wall-clock value may leak in.
_ARCHIVE_MTIME = 0

#: Executable permissions for the packaged binary. Cargo's own output mode
#: varies with the runner's umask, which would otherwise change the checksum.
_BINARY_MODE = 0o755


@dataclass(frozen=True)
class ReleaseArchiveSpec:
    """Inputs describing one target's release archive.

    Attributes
    ----------
    repo : Path
        Repository root holding Cargo's `target` directory.
    target : str
        Rust target triple, used in the archive name and the build output path.
    version : str
        Package version without the leading `v`.
    dist_dir : Path
        Directory that receives the archive and its checksum sidecar.
    """

    repo: Path
    target: str
    version: str
    dist_dir: Path


def cargo_target_root(repo: Path) -> Path:
    """Return Cargo's target directory for a build rooted at *repo*.

    `CARGO_TARGET_DIR` is honoured so the packager finds the binary when a
    caller redirects Cargo's output, which agents and shared build caches do.
    A relative override is resolved against *repo*, matching Cargo itself.
    """
    override = os.environ.get("CARGO_TARGET_DIR", "").strip()
    if not override:
        return repo / "target"
    override_path = Path(override).expanduser()
    return override_path if override_path.is_absolute() else repo / override_path


def release_binary_path(repo: Path, target: str) -> Path:
    """Return where Cargo writes the release binary for *target*.

    Examples
    --------
    >>> release_binary_path(Path("/w"), "x86_64-pc-windows-msvc").name
    'cargo-orthohelp.exe'
    """
    name = f"{PACKAGE_NAME}{binary_extension(target)}"
    return cargo_target_root(repo) / target / "release" / name


def build_release_binary(repo: Path, target: str, manifest_path: Path, cargo: str) -> None:
    """Build the release binary for *target*.

    Raises
    ------
    SystemExit
        Raised when Cargo exits non-zero, so the workflow step fails loudly
        rather than packaging a stale binary from an earlier run.
    """
    command = [
        cargo,
        "build",
        "--release",
        "--locked",
        "--manifest-path",
        str(manifest_path),
        "--target",
        target,
        "--bin",
        PACKAGE_NAME,
    ]
    # The S603 suppression below is safe: the argument list is built here from
    # a validated target triple and the caller's cargo executable, and no shell
    # interprets it.
    result = subprocess.run(command, cwd=repo, check=False)  # noqa: S603
    if result.returncode != 0:
        message = f"cargo build failed for {target} with exit code {result.returncode}"
        raise SystemExit(message)


def _binary_tarinfo(member_path: str, size: int) -> tarfile.TarInfo:
    """Return a normalized `TarInfo` for the packaged binary."""
    info = tarfile.TarInfo(member_path)
    info.size = size
    info.mode = _BINARY_MODE
    info.mtime = _ARCHIVE_MTIME
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.type = tarfile.REGTYPE
    return info


def stage_archive(spec: ReleaseArchiveSpec) -> Path:
    """Write the release archive for *spec* and return its path.

    The archive holds a single member, `<stem>/<binary>`, matching the
    `bin-dir` template. It is byte-for-byte reproducible: member metadata and
    the gzip header timestamp are fixed.

    Raises
    ------
    SystemExit
        Raised when the release binary is missing, which means the build step
        did not run or targeted a different triple.
    """
    binary_path = release_binary_path(spec.repo, spec.target)
    if not binary_path.is_file():
        message = f"release binary not found at {binary_path}"
        raise SystemExit(message)

    spec.dist_dir.mkdir(parents=True, exist_ok=True)
    archive_path = spec.dist_dir / archive_file_name(PACKAGE_NAME, spec.target, spec.version)
    member_path = (
        f"{archive_stem(PACKAGE_NAME, spec.target, spec.version)}/{binary_path.name}"
    )
    payload = binary_path.read_bytes()

    with archive_path.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=_ARCHIVE_MTIME) as gz:
            with tarfile.open(fileobj=gz, mode="w", format=tarfile.GNU_FORMAT) as tar:
                tar.addfile(_binary_tarinfo(member_path, len(payload)), io.BytesIO(payload))
    return archive_path


def write_checksum_sidecar(archive_path: Path) -> Path:
    """Write a `sha256sum`-compatible sidecar beside *archive_path*.

    The sidecar names the archive without a directory component so that
    `sha256sum --check` succeeds when run from the directory holding both.
    """
    digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    sidecar_path = archive_path.with_name(f"{archive_path.name}.sha256")
    sidecar_path.write_text(f"{digest}  {archive_path.name}\n", encoding="utf-8")
    return sidecar_path


def resolve_version(manifest_path: Path, requested: str | None) -> str:
    """Return the release version, rejecting a mismatch with the manifest.

    Raises
    ------
    SystemExit
        Raised when the manifest is unreadable or the requested version does
        not match it, which would publish an asset the `pkg-url` template
        cannot resolve.
    """
    try:
        declared = manifest_version(manifest_path)
    except ManifestError as err:
        raise SystemExit(str(err)) from err
    if requested is not None and requested != declared:
        message = (
            f"requested release version {requested} does not match the "
            f"{manifest_path} package version {declared}"
        )
        raise SystemExit(message)
    return declared


def _parse_args(argv: list[str] | None) -> argparse.Namespace:
    """Parse the command-line arguments for the packager."""
    parser = argparse.ArgumentParser(description="Package a cargo-orthohelp release archive.")
    parser.add_argument("target", help="Rust target triple to package.")
    parser.add_argument(
        "--release-version",
        default=None,
        help="Release version without the leading v; must match the manifest.",
    )
    parser.add_argument(
        "--dist-dir",
        type=Path,
        default=Path("dist"),
        help="Directory that receives the archive and its checksum sidecar.",
    )
    parser.add_argument(
        "--manifest-path",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="Path to the cargo-orthohelp Cargo.toml.",
    )
    parser.add_argument("--cargo", default="cargo", help="Cargo executable to invoke.")
    parser.add_argument(
        "--skip-build",
        action="store_true",
        help="Package an already-built binary instead of invoking Cargo.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    """Build, package, and checksum one release archive.

    Raises
    ------
    SystemExit
        Raised on manifest, build, or staging failures.

    Examples
    --------
    >>> main(["x86_64-unknown-linux-gnu"])  # doctest: +SKIP
    """
    args = _parse_args(argv)
    repo = Path(os.environ.get("RELEASE_ARCHIVE_REPO", ".")).resolve()
    manifest_path = args.manifest_path
    if not manifest_path.is_absolute():
        manifest_path = repo / manifest_path
    version = resolve_version(manifest_path, args.release_version)

    if not args.skip_build:
        build_release_binary(repo, args.target, manifest_path, args.cargo)

    dist_dir = args.dist_dir if args.dist_dir.is_absolute() else repo / args.dist_dir
    archive_path = stage_archive(
        ReleaseArchiveSpec(repo=repo, target=args.target, version=version, dist_dir=dist_dir)
    )
    write_checksum_sidecar(archive_path)
    print(archive_path)


if __name__ == "__main__":
    main()
