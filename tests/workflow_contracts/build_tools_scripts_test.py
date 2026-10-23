"""Contract for the build-tools scripts behind ``install-build-tools`` and ``check-build-tools``.

A missing prerequisite has to produce an actionable installation error, not a
silent fallback to a slower build and not an opaque linker failure deep inside
Cargo. The scripts are driven here with fixtures standing in for the network,
the linker and ``rustup``, so every refusal is observed rather than read:

- ``check-build-tools`` fails, and says ``make install-build-tools``, when the
  linker is absent, cannot run, or differs from the pin;
- ``install-build-tools`` refuses an archive whose digest is not the recorded
  one, refuses an artefact with no recorded digest, installs nothing in either
  case, and on success installs under the requested prefix and then asks
  ``rustup`` for the toolchain the repository names.

The scripts are Linux-first by design: on any other host they skip the linker
and say so. This contract runs on Linux, as does the CI step that carries it.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import hashlib
import os
import pathlib
import shutil
import subprocess  # noqa: S404 - the contract drives the scripts, which are the subject.
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
    """Write an executable shell script called *name* into *directory*."""
    path = directory / name
    path.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
    path.chmod(0o755)


def _run(script: pathlib.Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    """Run a script with exactly the environment given, plus the system tools."""
    return subprocess.run(  # noqa: S603 - fixed argument vector, no shell.
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


def _fake_rustup(directory: pathlib.Path, *, installed: bool = True) -> None:
    """Install a ``rustup`` that reports the pinned toolchain as present or absent.

    The toolchain half of the check is not under test in the linker cases, so it
    is made to pass rather than allowed to fail for an unrelated reason: a case
    that expects a non-zero exit would otherwise be satisfied by the wrong half.
    """
    listing = f"{_pinned_channel()}-x86_64-unknown-linux-gnu (active)" if installed else ""
    _tool(
        directory,
        "rustup",
        'if [ "$1" = toolchain ]; then echo "' + listing + '"; fi',
    )


def _pin(tmp_path: pathlib.Path, version: str = PIN) -> pathlib.Path:
    """Write a one-line version pin and return its path."""
    pin = tmp_path / "VERSION"
    pin.write_text(f"{version}\n", encoding="utf-8")
    return pin


@pytest.fixture
def tools(tmp_path: pathlib.Path) -> pathlib.Path:
    """Return a directory holding the system tools the scripts use, and nothing else.

    Linking the real binaries in, rather than inheriting ``PATH``, is what lets a
    case control whether ``mold`` or ``rustup`` exists at all.
    """
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


def _check(
    tmp_path: pathlib.Path, tools: pathlib.Path, *, version: str = PIN
) -> subprocess.CompletedProcess[str]:
    """Run the check against the fixture pin with the fixture tools only."""
    return _run(
        CHECK,
        {"PATH": str(tools), "MOLD_VERSION_FILE": str(_pin(tmp_path, version))},
    )


def test_check_passes_when_every_prerequisite_is_present(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """The positive control: the failures below are about their one missing piece."""
    _fake_rustup(tools)
    _tool(tools, "mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    result = _check(tmp_path, tools)
    assert result.returncode == 0, result.stderr


def test_check_fails_with_the_install_hint_when_mold_is_missing(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """No linker on ``PATH``: the check fails, names the fix, and blames only the linker."""
    _fake_rustup(tools)
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "mold not found on PATH" in result.stderr, result.stderr
    assert HINT in result.stderr, result.stderr
    assert "is not installed" not in result.stderr, result.stderr


def test_check_fails_when_mold_cannot_report_its_version(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """A truncated or broken linker is a failure, not a version-drift warning."""
    _fake_rustup(tools)
    _tool(tools, "mold", "exit 1")
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "cannot report its version" in result.stderr, result.stderr
    assert HINT in result.stderr, result.stderr


def test_check_fails_when_mold_differs_from_the_pin(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """An advisory pin is not a pin: a different release fails, and says how to match."""
    _fake_rustup(tools)
    _tool(tools, "mold", "echo 'mold 1.0.0 (compatible with GNU ld)'")
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert f"does not match the pin {PIN}" in result.stderr, result.stderr
    assert HINT in result.stderr, result.stderr


def test_check_fails_when_the_pinned_toolchain_is_not_installed(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """The toolchain half fails on its own, with the same fix named."""
    _fake_rustup(tools, installed=False)
    _tool(tools, "mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "is not installed" in result.stderr, result.stderr
    assert HINT in result.stderr, result.stderr
    assert "mold not found" not in result.stderr, result.stderr


def test_check_reports_every_missing_piece_in_one_pass(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """The linker and rustup are both reported, so one run shows the whole gap."""
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "rustup not found on PATH" in result.stderr, result.stderr
    assert "mold not found on PATH" in result.stderr, result.stderr


def test_check_refuses_an_unreadable_pin(tmp_path: pathlib.Path, tools: pathlib.Path) -> None:
    """A missing pin file aborts rather than checking against an empty version."""
    result = _run(CHECK, {"PATH": str(tools), "MOLD_VERSION_FILE": str(tmp_path / "absent")})
    assert result.returncode != 0
    assert "missing version pin" in result.stderr, result.stderr


@pytest.mark.parametrize(
    ("version", "diagnostic"),
    [
        pytest.param("", "empty version pin", id="blank"),
        pytest.param("9.9.9\n9.9.9", "expected one line", id="multiple-lines"),
        pytest.param("9.9 .9", "version pin contains whitespace", id="internal-whitespace"),
    ],
)
def test_check_refuses_malformed_version_pins(
    tmp_path: pathlib.Path, tools: pathlib.Path, version: str, diagnostic: str
) -> None:
    """The check rejects blank, multiline and internally spaced pins."""
    _fake_rustup(tools)
    _tool(tools, "mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    result = _check(tmp_path, tools, version=version)
    assert result.returncode != 0, result.stderr
    assert diagnostic in result.stderr, result.stderr


class OfflineInstall(typ.NamedTuple):
    """Everything an installer case needs: where it runs and what it may fetch."""

    root: pathlib.Path
    tools: pathlib.Path
    release_dir: pathlib.Path
    sums_line: str

    def env(self, sums: str) -> dict[str, str]:
        """Return an environment that points the installer at fixtures only.

        Parameters
        ----------
        sums : str
            The contents of the checksum file the installer should trust.
        """
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
def offline_install(tmp_path: pathlib.Path, tools: pathlib.Path) -> OfflineInstall:
    """Build a mold-shaped tarball for this host and a stand-in ``rustup`` that logs."""
    arch = os.uname().machine
    name = f"mold-{PIN}-{arch}-linux.tar.gz"
    release_dir = tmp_path / "release"
    (release_dir / f"v{PIN}").mkdir(parents=True)
    root = tmp_path / "src" / f"mold-{PIN}-{arch}-linux"
    (root / "bin").mkdir(parents=True)
    (root / "bin" / "mold").write_text("#!/bin/sh\necho mold\n", encoding="utf-8")
    archive_path = release_dir / f"v{PIN}" / name
    with tarfile.open(archive_path, "w:gz") as archive:
        archive.add(root, arcname=root.name)
    digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    _tool(tools, "rustup", f"echo \"$@\" >> '{tmp_path}/rustup.log'")
    return OfflineInstall(tmp_path, tools, release_dir, f"{digest}  {name}\n")


def test_install_refuses_an_archive_with_the_wrong_digest(offline_install: OfflineInstall) -> None:
    """A tampered or corrupted download installs nothing."""
    wrong = "0" * 64 + offline_install.sums_line[64:]
    result = _run(INSTALL, offline_install.env(wrong))
    assert result.returncode != 0, result.stderr
    assert "checksum mismatch" in result.stderr, result.stderr
    assert not (offline_install.root / "prefix" / "bin").exists(), "an unverified archive was unpacked"
    assert not (offline_install.root / "rustup.log").exists(), (
        "the toolchain step ran after a failed verification"
    )


def test_install_refuses_an_artefact_with_no_recorded_digest(offline_install: OfflineInstall) -> None:
    """An unlisted artefact is a hard failure, never a silent skip."""
    result = _run(INSTALL, offline_install.env(""))
    assert result.returncode != 0, result.stderr
    assert "no checksum recorded" in result.stderr, result.stderr
    assert not (offline_install.root / "prefix" / "bin").exists()


def test_install_refuses_duplicate_checksum_rows_before_unpack_or_rustup(
    offline_install: OfflineInstall,
) -> None:
    """Ambiguous matching checksum rows stop before installation side effects."""
    duplicate = offline_install.sums_line * 2
    result = _run(INSTALL, offline_install.env(duplicate))
    assert result.returncode != 0, result.stderr
    assert "2 checksums recorded" in result.stderr, result.stderr
    assert not (offline_install.root / "prefix" / "bin").exists(), (
        "an archive with ambiguous checksums was unpacked"
    )
    assert not (offline_install.root / "rustup.log").exists(), (
        "rustup ran after checksum ambiguity was detected"
    )


def test_install_unpacks_a_verified_archive_then_installs_the_toolchain(
    offline_install: OfflineInstall,
) -> None:
    """The verified tree lands under the prefix, and rustup is asked for the pinned toolchain."""
    result = _run(INSTALL, offline_install.env(offline_install.sums_line))
    assert result.returncode == 0, result.stderr
    assert (offline_install.root / "prefix" / "bin" / "mold").is_file(), result.stderr
    logged = (offline_install.root / "rustup.log").read_text(encoding="utf-8").split()
    assert logged == ["toolchain", "install"], (
        "rustup was not asked to install the toolchain rust-toolchain.toml names"
    )


def test_the_scripts_and_pins_are_checked_out_with_lf_line_endings() -> None:
    """A CRLF checkout would make bash read each command as ``command\\r``.

    The Windows leg reaches the preflight through ``make lint-clippy``, and a
    shell script or pin file with the wrong line endings fails there with an
    error that names neither the file nor the cause, so the attribute is held.
    """
    rules = {
        line.split()[0]: line.split()[1:]
        for line in (REPOSITORY_ROOT / ".gitattributes").read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.startswith("#")
    }
    for pattern in ("*.sh", "tools/mold/VERSION", "tools/mold/SHA256SUMS"):
        assert rules.get(pattern) == ["text", "eol=lf"], f".gitattributes gives {pattern} {rules.get(pattern)}"
