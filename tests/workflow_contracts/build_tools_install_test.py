"""Contract for the pinned build-tools installer.

Offline fixtures prove that the installer selects the archive for each
supported machine name, verifies it before unpacking, installs only the pinned
release and refuses unsupported architectures without side effects.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib

import pytest
from build_tools_test_support import (
    INSTALL,
    OfflineInstall,
    _fake_uname,
    _pinned_channel,
    _pinned_components,
    _pinned_profile,
    _run,
    _tool,
    offline_install,
    tools,
)


def _quoting_scratch(root: pathlib.Path) -> pathlib.Path:
    """Return an empty ``TMPDIR`` whose path only works when it stays quoted."""
    scratch = root / "scratch 'quoted' dir"
    scratch.mkdir()
    return scratch


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


@pytest.mark.parametrize(
    "offline_install",
    [
        pytest.param(("x86_64", "x86_64"), id="x86_64"),
        pytest.param(("amd64", "x86_64"), id="amd64-alias"),
        pytest.param(("aarch64", "aarch64"), id="aarch64"),
        pytest.param(("arm64", "aarch64"), id="arm64-alias"),
    ],
    indirect=True,
)
def test_install_unpacks_a_verified_archive_then_installs_the_toolchain(
    offline_install: OfflineInstall,
) -> None:
    """Install the pinned channel and every configured component despite an override."""
    result = _run(
        INSTALL,
        {
            **offline_install.env(offline_install.sums_line),
            "RUSTUP_TOOLCHAIN": "stable",
        },
    )
    assert result.returncode == 0, result.stderr
    assert offline_install.archive_name in result.stderr, result.stderr
    assert (offline_install.root / "prefix" / "bin" / "ld.mold").is_file(), result.stderr
    logged = (offline_install.root / "rustup.log").read_text(encoding="utf-8").splitlines()
    assert logged == [
        "toolchain",
        "install",
        _pinned_channel(),
        "--profile",
        _pinned_profile(),
        *[argument for component in _pinned_components() for argument in ("--component", component)],
    ], "rustup did not receive the pinned channel and all configured components explicitly"


@pytest.mark.parametrize(
    "offline_install",
    [pytest.param(("s390x", "x86_64"), id="s390x")],
    indirect=True,
)
def test_install_refuses_unsupported_architecture_without_side_effects(
    offline_install: OfflineInstall,
) -> None:
    """An unknown machine fails before downloading or installing anything."""
    result = _run(INSTALL, offline_install.env(offline_install.sums_line))
    assert result.returncode != 0, result.stderr
    assert "unsupported architecture for the pinned mold release: s390x" in result.stderr
    assert "downloading" not in result.stderr
    assert not (offline_install.root / "prefix" / "bin").exists()
    assert not (offline_install.root / "rustup.log").exists()


def test_install_fails_when_host_os_cannot_be_detected(
    offline_install: OfflineInstall,
) -> None:
    """The installer must stop before skipping mold or installing the toolchain."""
    _tool(offline_install.tools, "uname", "exit 1")
    result = _run(INSTALL, offline_install.env(offline_install.sums_line))
    assert result.returncode != 0, result.stderr
    assert "could not determine host operating system using uname -s" in result.stderr
    assert not (offline_install.root / "prefix" / "bin").exists()
    assert not (offline_install.root / "rustup.log").exists()


def test_install_skips_mold_on_a_non_linux_host(offline_install: OfflineInstall) -> None:
    """A non-Linux host downloads no Linux linker and still installs the toolchain."""
    _fake_uname(offline_install.tools, machine="arm64", system="Darwin")
    result = _run(INSTALL, offline_install.env(offline_install.sums_line))
    assert result.returncode == 0, result.stderr
    assert "mold is Linux-only; skipping on Darwin" in result.stderr, result.stderr
    assert "downloading" not in result.stderr, "a Linux-only archive was fetched"
    assert not (offline_install.root / "prefix" / "bin").exists(), (
        "mold must not be unpacked on an unsupported host"
    )
    assert (offline_install.root / "rustup.log").is_file(), (
        "skipping the linker must not skip the toolchain"
    )


def test_install_removes_its_scratch_directory_after_success(
    offline_install: OfflineInstall,
) -> None:
    """A quote-bearing ``TMPDIR`` must not break the cleanup trap."""
    scratch = _quoting_scratch(offline_install.root)
    result = _run(
        INSTALL,
        {**offline_install.env(offline_install.sums_line), "TMPDIR": str(scratch)},
    )
    assert result.returncode == 0, result.stderr
    assert (offline_install.root / "prefix" / "bin" / "ld.mold").is_file(), result.stderr
    assert list(scratch.iterdir()) == [], "a successful install left its scratch directory behind"


def test_install_removes_its_scratch_directory_after_failure(
    offline_install: OfflineInstall,
) -> None:
    """The cleanup trap must run on the failing path too."""
    scratch = _quoting_scratch(offline_install.root)
    wrong = "0" * 64 + offline_install.sums_line[64:]
    result = _run(INSTALL, {**offline_install.env(wrong), "TMPDIR": str(scratch)})
    assert result.returncode != 0, result.stderr
    assert "checksum mismatch" in result.stderr, result.stderr
    assert list(scratch.iterdir()) == [], "a failed install left its scratch directory behind"
