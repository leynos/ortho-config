"""Fixtures owned by the build-tools script contract tests.

Only ``build_tools_scripts_test.py`` and ``build_tools_install_test.py`` should
use these Linux-only fake tools and offline archive data. They are scoped to
preflight and installer contracts, not general subprocess testing.
"""

from __future__ import annotations

import hashlib
import pathlib
import shlex
import shutil
import subprocess
import tarfile
import typing as typ

import pytest
from build_standard_make import REPOSITORY_ROOT

SCRIPTS: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "scripts"
CHECK: typ.Final[pathlib.Path] = SCRIPTS / "check-build-tools.sh"
INSTALL: typ.Final[pathlib.Path] = SCRIPTS / "install-build-tools.sh"
HINT: typ.Final[str] = "make install-build-tools"
PIN: typ.Final[str] = "9.9.9"


def _tool(directory: pathlib.Path, name: str, body: str) -> None:
    """Write an executable shell fixture, replacing any linked host tool."""
    path = directory / name
    if path.is_symlink():
        path.unlink()
    path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    path.chmod(0o755)


def _run(script: pathlib.Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    """Run a build-tools script with only the supplied environment and host path."""
    return subprocess.run(  # noqa: S603 - fixed argument vector, script is the subject.
        ["bash", str(script)],
        capture_output=True,
        text=True,
        check=False,
        env={"HOME": env.get("HOME", "/nonexistent"), **env},
    )


def _pinned_channel() -> str:
    """Return the channel ``rust-toolchain.toml`` pins, as the scripts read it."""
    for line in (REPOSITORY_ROOT / "rust-toolchain.toml").read_text(encoding="utf-8").splitlines():
        if line.strip().startswith("channel"):
            return line.split('"')[1]
    message = "rust-toolchain.toml names no channel"
    raise AssertionError(message)


def _pinned_components() -> list[str]:
    """Return the component names in the toolchain's multiline TOML array."""
    lines = (REPOSITORY_ROOT / "rust-toolchain.toml").read_text(encoding="utf-8").splitlines()
    component_lines: list[str] = []
    in_components = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("components"):
            in_components = True
        elif in_components and stripped == "]":
            break
        elif in_components and stripped.startswith('"'):
            component_lines.append(stripped.split('"')[1])
    return component_lines


def _pinned_profile() -> str:
    """Return the profile in the toolchain's TOML section."""
    for line in (REPOSITORY_ROOT / "rust-toolchain.toml").read_text(encoding="utf-8").splitlines():
        if line.strip().startswith("profile"):
            return line.split('"')[1]
    message = "rust-toolchain.toml names no profile"
    raise AssertionError(message)


def _fake_rustup(
    directory: pathlib.Path, *, installed: bool = True, cranelift_installed: bool = True
) -> None:
    """Install fake ``rustup`` with configurable toolchain and backend presence."""
    listing = f"{_pinned_channel()}-x86_64-unknown-linux-gnu (active)" if installed else ""
    component = "rustc-codegen-cranelift-x86_64-unknown-linux-gnu" if cranelift_installed else ""
    _tool(directory, "rustup", "\n".join([
        'if [ "$1" = toolchain ]; then echo "' + listing + '"; fi',
        'if [ "$1" = component ]; then echo "' + component + '"; fi',
    ]))


def _fake_uname(directory: pathlib.Path, *, machine: str, system: str = "Linux") -> None:
    """Install ``uname`` that reports the requested OS and machine values."""
    _tool(directory, "uname", "\n".join([
        'case "$1" in',
        f"  -s) printf '%s\\n' {shlex.quote(system)} ;;",
        f"  -m) printf '%s\\n' {shlex.quote(machine)} ;;",
        "  *) exit 2 ;;",
        "esac",
    ]))


@pytest.fixture
def tools(tmp_path: pathlib.Path) -> pathlib.Path:
    """Return Linux test tools with controlled linker, rustup and uname entries."""
    import sys

    if not sys.platform.startswith("linux"):
        pytest.skip("the build-tools script fixtures require Linux")
    directory = tmp_path / "bin"
    directory.mkdir()
    needed = (
        "awk", "bash", "cat", "curl", "dirname", "grep", "gzip", "mkdir", "mktemp",
        "rm", "sha256sum", "tar", "uname",
    )
    for name in needed:
        found = shutil.which(name)
        assert found, f"the host has no {name}, which the scripts under test need"
        (directory / name).symlink_to(found)
    return directory


def _pin(tmp_path: pathlib.Path, version: str = PIN) -> pathlib.Path:
    """Write a one-line version pin and return its path."""
    pin = tmp_path / "VERSION"
    pin.write_text(f"{version}\n", encoding="utf-8")
    return pin


class OfflineInstall(typ.NamedTuple):
    """Everything an installer case needs: where it runs and what it may fetch."""

    root: pathlib.Path
    tools: pathlib.Path
    release_dir: pathlib.Path
    sums_line: str
    archive_name: str

    def env(self, sums: str) -> dict[str, str]:
        """Return an environment that points the installer at fixtures only."""
        sums_file = self.root / "SHA256SUMS"
        sums_file.write_text(sums, encoding="utf-8")
        return {
            "PATH": str(self.tools),
            "MOLD_VERSION_FILE": str(_pin(self.root)),
            "MOLD_SHA256SUMS_FILE": str(sums_file),
            "MOLD_RELEASE_BASE_URL": f"file://{self.release_dir}",
            "BUILD_TOOLS_PREFIX": str(self.root / "prefix"),
        }


@pytest.fixture
def offline_install(
    tmp_path: pathlib.Path, tools: pathlib.Path, request: pytest.FixtureRequest
) -> OfflineInstall:
    """Build a pinned mold archive for a requested machine and fake rustup."""
    machine, archive_arch = getattr(request, "param", ("x86_64", "x86_64"))
    _fake_uname(tools, machine=machine)
    archive_name = f"mold-{PIN}-{archive_arch}-linux.tar.gz"
    release_dir = tmp_path / "release"
    (release_dir / f"v{PIN}").mkdir(parents=True)
    root = tmp_path / "src" / f"mold-{PIN}-{archive_arch}-linux"
    (root / "bin").mkdir(parents=True)
    (root / "bin" / "ld.mold").write_text("#!/bin/sh\necho mold\n", encoding="utf-8")
    archive_path = release_dir / f"v{PIN}" / archive_name
    with tarfile.open(archive_path, "w:gz") as archive:
        archive.add(root, arcname=root.name)
    digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    _tool(tools, "rustup", f"printf '%s\\n' \"$@\" >> '{tmp_path}/rustup.log'")
    return OfflineInstall(tmp_path, tools, release_dir, f"{digest}  {archive_name}\n", archive_name)
