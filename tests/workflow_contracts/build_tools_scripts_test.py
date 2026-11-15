"""Contract for the build-tools preflight and pinned-file line endings.

The preflight must report missing, broken or mismatched tools before a Cargo
command runs. Its fixtures replace the host linker and rustup so each refusal
is observed directly.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import subprocess
from typing import Literal

import pytest
from build_standard_make import REPOSITORY_ROOT
from build_tools_test_support import (
    CHECK,
    HINT,
    PIN,
    _fake_rustup,
    _pin,
    _run,
    _tool,
    tools,
)


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
    _tool(tools, "ld.mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    result = _check(tmp_path, tools)
    assert result.returncode == 0, result.stderr
    assert "Cranelift codegen component available" in result.stderr, result.stderr


def test_check_fails_with_the_install_hint_when_cranelift_is_missing(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """A nightly without its selected development backend is not ready to build."""
    _fake_rustup(tools, cranelift_installed=False)
    _tool(tools, "ld.mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "Cranelift codegen component is not installed" in result.stderr, result.stderr
    assert HINT in result.stderr, result.stderr


@pytest.mark.parametrize(
    "missing",
    [
        pytest.param("linker", id="ld-mold-missing"),
        pytest.param("toolchain", id="toolchain-missing"),
    ],
)
def test_check_fails_with_the_install_hint_for_a_missing_prerequisite(
    tmp_path: pathlib.Path, tools: pathlib.Path, missing: Literal["linker", "toolchain"]
) -> None:
    """Each missing prerequisite is diagnosed without masking the other check."""
    if missing == "linker":
        _fake_rustup(tools)
        _tool(tools, "mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
        diagnostic = "ld.mold not found on PATH"
        absent_diagnostic = "is not installed"
    else:
        _fake_rustup(tools, installed=False)
        _tool(tools, "ld.mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
        diagnostic = "is not installed"
        absent_diagnostic = "ld.mold not found"
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert diagnostic in result.stderr, result.stderr
    assert HINT in result.stderr, result.stderr
    assert absent_diagnostic not in result.stderr, result.stderr


def test_check_fails_when_mold_cannot_report_its_version(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """A truncated or broken linker is a failure, not a version-drift warning."""
    _fake_rustup(tools)
    _tool(tools, "ld.mold", "exit 1")
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "cannot report its version" in result.stderr, result.stderr
    assert HINT in result.stderr, result.stderr


def test_check_fails_when_mold_differs_from_the_pin(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """An advisory pin is not a pin: a different release fails, and says how to match."""
    _fake_rustup(tools)
    _tool(tools, "mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    _tool(tools, "ld.mold", "echo 'mold 1.0.0 (compatible with GNU ld)'")
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert f"does not match the pin {PIN}" in result.stderr, result.stderr
    assert HINT in result.stderr, result.stderr


def test_check_reports_every_missing_piece_in_one_pass(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """The linker and rustup are both reported, so one run shows the whole gap."""
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "rustup not found on PATH" in result.stderr, result.stderr
    assert "ld.mold not found on PATH" in result.stderr, result.stderr


def test_check_fails_when_host_os_cannot_be_detected(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """A failed uname query must not masquerade as a non-Linux linker skip."""
    _fake_rustup(tools)
    _tool(tools, "ld.mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    _tool(tools, "uname", "exit 1")
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "could not determine host operating system using uname -s" in result.stderr


def test_check_preserves_rustup_toolchain_list_failure(
    tmp_path: pathlib.Path, tools: pathlib.Path
) -> None:
    """A failed rustup listing is not misreported as an absent toolchain."""
    _tool(tools, "ld.mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    _tool(tools, "rustup", 'if [ "$1" = toolchain ]; then exit 17; fi')
    result = _check(tmp_path, tools)
    assert result.returncode != 0, result.stderr
    assert "could not list installed Rust toolchains" in result.stderr, result.stderr
    assert "is not installed" not in result.stderr, result.stderr


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
    _tool(tools, "ld.mold", f"echo 'mold {PIN} (compatible with GNU ld)'")
    result = _check(tmp_path, tools, version=version)
    assert result.returncode != 0, result.stderr
    assert diagnostic in result.stderr, result.stderr

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
