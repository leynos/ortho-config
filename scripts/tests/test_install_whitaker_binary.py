"""Behavioural tests for the direct Whitaker release-asset installer.

The tests build a tiny local release archive and route the helper's GitHub API
calls through a fake ``gh`` command. No test downloads an asset or writes to a
user Cargo home, while the helper still executes its cold-cache, verification,
extraction, and installation paths.
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import stat
import subprocess
import tarfile
import typing as typ
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
INSTALLER = REPOSITORY_ROOT / "scripts" / "install_whitaker_binary.sh"
VERSION = "0.2.8"
TARGET = "x86_64-unknown-linux-gnu"
ARCHIVE_NAME = f"whitaker-installer-{TARGET}-v{VERSION}.tgz"
CHECKSUM_NAME = f"{ARCHIVE_NAME}.sha256"
MEMBER_NAME = f"whitaker-installer-{TARGET}-v{VERSION}/whitaker-installer"
ARCHIVE_ID = "101"
CHECKSUM_ID = "102"
BINARY_MODE = 0o755


def _sha256(path: Path) -> str:
    """Return the SHA-256 digest of *path*."""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _write_release_assets(directory: Path) -> dict[str, Path]:
    """Create a small release archive, sidecar, and matching API document."""
    staged_binary = directory / MEMBER_NAME
    staged_binary.parent.mkdir(parents=True)
    staged_binary.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    staged_binary.chmod(BINARY_MODE)
    archive = directory / ARCHIVE_NAME
    with tarfile.open(archive, mode="w:gz") as tar:
        tar.add(staged_binary, arcname=MEMBER_NAME)
    checksum = directory / CHECKSUM_NAME
    checksum.write_text(f"{_sha256(archive)}  {ARCHIVE_NAME}\n", encoding="utf-8")
    release = directory / "release.json"
    release.write_text(
        json.dumps(
            {
                "assets": [
                    {
                        "id": ARCHIVE_ID,
                        "name": ARCHIVE_NAME,
                        "state": "uploaded",
                        "digest": f"sha256:{_sha256(archive)}",
                    },
                    {
                        "id": CHECKSUM_ID,
                        "name": CHECKSUM_NAME,
                        "state": "uploaded",
                        "digest": f"sha256:{_sha256(checksum)}",
                    },
                ]
            }
        ),
        encoding="utf-8",
    )
    return {
        "archive": archive,
        "binary": staged_binary,
        "checksum": checksum,
        "release": release,
    }


def _write_fake_gh(directory: Path) -> Path:
    """Create a GitHub API stand-in driven by files in its environment."""
    command = directory / "gh"
    command.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
[[ "$1" == "api" ]] || exit 64
shift
if [[ "$1" == "-H" ]]; then
    shift 2
fi
endpoint="$1"
printf '%s|%s|%s\\n' "$endpoint" "${GH_TOKEN:+present}" "${GITHUB_TOKEN:+present}" >> "$GH_CALLS"
case "$endpoint" in
    repos/leynos/whitaker/releases/tags/v0.2.8)
        cat "$FAKE_RELEASE"
        ;;
    repos/leynos/whitaker/releases/assets/101)
        [[ "${DENY_ASSET_DOWNLOADS:-}" != "1" ]] || exit 70
        cat "$FAKE_ARCHIVE"
        ;;
    repos/leynos/whitaker/releases/assets/102)
        [[ "${DENY_ASSET_DOWNLOADS:-}" != "1" ]] || exit 70
        cat "$FAKE_CHECKSUM"
        ;;
    *) exit 65 ;;
esac
""",
        encoding="utf-8",
    )
    command.chmod(command.stat().st_mode | stat.S_IXUSR)
    return command


@pytest.fixture
def install_environment(tmp_path: Path) -> dict[str, str]:
    """Return an isolated environment backed by a synthetic release."""
    assets = _write_release_assets(tmp_path / "release")
    fake_bin = tmp_path / "bin"
    fake_bin.mkdir()
    _write_fake_gh(fake_bin)
    return {
        "PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}",
        "HOME": str(tmp_path / "home"),
        "CARGO_HOME": str(tmp_path / "cargo"),
        "XDG_CACHE_HOME": str(tmp_path / "cache"),
        "WHITAKER_INSTALLER_VERSION": VERSION,
        "RUNNER_ARCH": "X64",
        "INSTALL_TOKEN": "test-token",
        "FAKE_ARCHIVE": str(assets["archive"]),
        "FAKE_BINARY": str(assets["binary"]),
        "FAKE_CHECKSUM": str(assets["checksum"]),
        "FAKE_RELEASE": str(assets["release"]),
        "GH_CALLS": str(tmp_path / "gh-calls"),
    }


def _run_installer(environment: dict[str, str]) -> subprocess.CompletedProcess[str]:
    """Run the installer with Bash found from the test host's environment."""
    bash = shutil.which("bash")
    assert bash is not None, "the installer contract requires Bash"
    # S603: Bash and the helper are repository-controlled paths, not test input.
    return subprocess.run(  # noqa: S603
        [bash, str(INSTALLER)],
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )


def test_cold_install_verifies_and_installs_the_release_binary(
    install_environment: dict[str, str]
) -> None:
    """A cold cache installs a verified binary without a source toolchain."""
    result = _run_installer(install_environment)
    installed = Path(install_environment["CARGO_HOME"]) / "bin" / "whitaker-installer"
    calls = Path(install_environment["GH_CALLS"]).read_text(encoding="utf-8").splitlines()
    assert result.returncode == 0, result.stderr
    assert installed.is_file(), "the verified release member must enter the isolated Cargo home"
    assert installed.read_bytes() == Path(install_environment["FAKE_BINARY"]).read_bytes(), (
        "the installed binary must be the exact verified release member"
    )
    assert stat.S_IMODE(installed.stat().st_mode) == BINARY_MODE, (
        "the installed release member must have the expected executable mode"
    )
    assert len(calls) == 3, f"cold install must read metadata and two assets: {calls!r}"
    assert all(call.endswith("|present|") for call in calls), (
        f"only command-local GH_TOKEN must reach the GitHub client: {calls!r}"
    )


def test_valid_cached_assets_are_revalidated_without_a_second_download(
    install_environment: dict[str, str]
) -> None:
    """A cache hit uses the same verified assets and never trusts a bare binary."""
    first = _run_installer(install_environment)
    assert first.returncode == 0, first.stderr
    installed = Path(install_environment["CARGO_HOME"]) / "bin" / "whitaker-installer"
    installed.unlink()
    Path(install_environment["GH_CALLS"]).unlink()
    install_environment["DENY_ASSET_DOWNLOADS"] = "1"
    second = _run_installer(install_environment)
    calls = Path(install_environment["GH_CALLS"]).read_text(encoding="utf-8").splitlines()
    assert second.returncode == 0, second.stderr
    assert installed.read_bytes() == Path(install_environment["FAKE_BINARY"]).read_bytes(), (
        "a valid asset cache must recreate the verified release member"
    )
    assert stat.S_IMODE(installed.stat().st_mode) == BINARY_MODE, (
        "a cache-hit installation must restore the expected executable mode"
    )
    assert calls == ["repos/leynos/whitaker/releases/tags/v0.2.8|present|"], (
        f"cache validation must need metadata but no asset download: {calls!r}"
    )


def test_a_corrupt_cache_is_repaired_from_verified_release_assets(
    install_environment: dict[str, str]
) -> None:
    """A damaged cache is discarded before the expected member is extracted."""
    first = _run_installer(install_environment)
    assert first.returncode == 0, first.stderr
    archive = (
        Path(install_environment["XDG_CACHE_HOME"])
        / "ortho-config"
        / "whitaker-installer"
        / ARCHIVE_NAME
    )
    archive.write_bytes(b"corrupt archive")
    Path(install_environment["GH_CALLS"]).unlink()
    second = _run_installer(install_environment)
    calls = Path(install_environment["GH_CALLS"]).read_text(encoding="utf-8").splitlines()
    assert second.returncode == 0, second.stderr
    assert len(calls) == 3, f"corrupt cache must trigger fresh asset downloads: {calls!r}"


def test_a_bad_release_sidecar_fails_before_installing(
    install_environment: dict[str, str]
) -> None:
    """A sidecar with a valid API digest but bad archive digest is rejected."""
    checksum = Path(install_environment["FAKE_CHECKSUM"])
    checksum.write_text(f"{'0' * 64}  {ARCHIVE_NAME}\n", encoding="utf-8")
    release = Path(install_environment["FAKE_RELEASE"])
    metadata = json.loads(release.read_text(encoding="utf-8"))
    metadata["assets"][1]["digest"] = f"sha256:{_sha256(checksum)}"
    release.write_text(json.dumps(metadata), encoding="utf-8")
    result = _run_installer(install_environment)
    installed = Path(install_environment["CARGO_HOME"]) / "bin" / "whitaker-installer"
    assert result.returncode != 0, "a mismatched archive digest must fail closed"
    assert "failed verification" in result.stderr, result.stderr
    assert not installed.exists(), "a failed verification must not install a binary"


def test_a_missing_github_client_fails_clearly(install_environment: dict[str, str]) -> None:
    """The helper refuses to continue when it cannot fetch release metadata."""
    bash = shutil.which("bash")
    assert bash is not None, "the installer contract requires Bash"
    environment = {**install_environment, "PATH": str(Path(install_environment["GH_CALLS"]).parent)}
    # S603: Bash and the helper are repository-controlled paths, not test input.
    result = subprocess.run(  # noqa: S603
        [bash, str(INSTALLER)],
        capture_output=True,
        check=False,
        env=environment,
        text=True,
    )
    assert result.returncode != 0, "a missing GitHub client must fail closed"
    assert "required command is unavailable: gh" in result.stderr, result.stderr


def test_an_unsupported_architecture_fails_before_network_access(
    install_environment: dict[str, str]
) -> None:
    """Only release targets published for the Linux CI lane are accepted."""
    install_environment["RUNNER_ARCH"] = "s390x"
    result = _run_installer(install_environment)
    calls = Path(install_environment["GH_CALLS"])
    assert result.returncode != 0, "an unsupported target must fail closed"
    assert "unsupported Whitaker installer architecture: s390x" in result.stderr
    assert not calls.exists(), "target validation must precede release API access"
