"""Behavioural check of the coverage target-root verification steps.

`coverage_target_root_verification_test.py` reads the two verification steps as
workflow text. This module runs the same scripts under the shells the workflow
declares and supplies both a passing and a failing environment to each, so a
changed script is observed rather than modelled.

The Windows script derives its expected root from the drive containing
`RUNNER_TEMP`. On a POSIX host that drive is `/`, so its acceptance path needs a
writable drive root and is left to the hosted Windows leg, which runs the step
with the real directory present. Its two rejection paths are observed here on
every host. The Linux script reads `RUNNER_TEMP` directly and is exercised in
full, acceptance included.

The PowerShell scripts run only where `pwsh` is installed; the Linux coverage
leg installs it and runs this suite. The Linux verification step is plain Bash
and runs everywhere.

Run via `make test-workflow-contracts`.
"""

from __future__ import annotations

import os
import pathlib
import shutil
import subprocess  # noqa: S404 - the contract runs the declared shells directly
import typing as typ

import pytest

from coverage_target_root_verification_test import (
    DERIVED_DIRECTORY,
    LINUX_VERIFY_STEP,
    OBSERVED_LINE,
    WINDOWS_VERIFY_STEP,
    _build_test_steps,
    _one_step,
    _run as _run_block,
)

PWSH: typ.Final[str] = "pwsh"
PWSH_REQUIRED = pytest.mark.skipif(
    shutil.which(PWSH) is None,
    reason="PowerShell 7 is installed on the Linux coverage leg, not everywhere",
)

#: What the Windows check derives, evaluated by the shell it declares.
DERIVATION_PROBE: typ.Final[str] = (
    "Join-Path ([System.IO.Path]::GetPathRoot($env:RUNNER_TEMP)) 'llvm-cov-target'"
)


def _script(name: str) -> str:
    """Return one verification step's run block as a script to execute."""
    return _run_block(_one_step(_build_test_steps(), name))


def _environment(tmp_path: pathlib.Path) -> dict[str, str]:
    """Return a minimal runner environment rooted inside ``tmp_path``."""
    return {
        "PATH": os.environ.get("PATH", ""),
        "HOME": os.environ.get("HOME", str(tmp_path)),
        "RUNNER_TEMP": str(tmp_path / "runner_temp"),
        "GITHUB_WORKSPACE": str(tmp_path / "workspace"),
        "GITHUB_ENV": str(tmp_path / "github_env"),
        "GITHUB_STEP_SUMMARY": str(tmp_path / "step_summary"),
    }


def _completed(
    argv: list[str], script: str, environment: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run one script and return the completed process.

    The script is passed as an argument rather than on standard input: reading
    ``-Command -`` from a pipe makes PowerShell emit terminal control sequences
    around its output, which would have to be stripped back out.
    """
    return subprocess.run(  # noqa: S603 - fixed argv, no shell
        [*argv, script],
        env=environment,
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )


def _run_pwsh(script: str, environment: dict[str, str]) -> str:
    """Run one PowerShell script that must succeed and return its output."""
    completed = _completed(
        [PWSH, "-NoProfile", "-NonInteractive", "-Command"], script, environment
    )
    assert completed.returncode == 0, completed.stderr
    return completed.stdout


def _derived_root(environment: dict[str, str]) -> str:
    """Return the root the Windows check derives, as that shell computes it."""
    return _run_pwsh(DERIVATION_PROBE, environment).strip()


@PWSH_REQUIRED
@pytest.mark.parametrize(
    ("wrong_variable", "reported"),
    [
        ("CARGO_TARGET_DIR", "CARGO_TARGET_DIR"),
        ("CARGO_LLVM_COV_TARGET_DIR", "CARGO_LLVM_COV_TARGET_DIR"),
    ],
)
def test_the_windows_check_rejects_either_variable_going_astray(
    tmp_path: pathlib.Path, wrong_variable: str, reported: str
) -> None:
    """One variable is set as the chooser would leave it, the other is not.

    Supplying the correct value for the other variable keeps the first check
    from answering for the second: each rejection has to be reachable on its
    own or a single broken comparison would hide the other.
    """
    environment = _environment(tmp_path)
    derived = _derived_root(environment)
    supplied = {
        variable: derived
        for variable in ("CARGO_TARGET_DIR", "CARGO_LLVM_COV_TARGET_DIR")
    }
    supplied[wrong_variable] = str(tmp_path / "elsewhere")

    observed = _completed(
        [PWSH, "-NoProfile", "-NonInteractive", "-Command"],
        _script(WINDOWS_VERIFY_STEP),
        {**environment, **supplied},
    )

    assert observed.returncode != 0, (
        f"a wrong {wrong_variable} must fail the lane"
    )
    assert reported in observed.stderr, (
        f"the failure must name {reported}, not report {observed.stderr!r}"
    )


@PWSH_REQUIRED
def test_the_windows_check_requires_the_derived_directory(
    tmp_path: pathlib.Path,
) -> None:
    """Matching variables do not pass the check: the directory must exist."""
    environment = _environment(tmp_path)
    derived = _derived_root(environment)
    if pathlib.Path(derived).exists():
        pytest.skip(f"{derived} already exists on this host, so absence is untestable")
    supplied = {
        "CARGO_TARGET_DIR": derived,
        "CARGO_LLVM_COV_TARGET_DIR": derived,
    }

    observed = _completed(
        [PWSH, "-NoProfile", "-NonInteractive", "-Command"],
        _script(WINDOWS_VERIFY_STEP),
        {**environment, **supplied},
    )

    assert observed.returncode != 0, "an absent derived root must fail the lane"
    assert DERIVED_DIRECTORY in observed.stderr, (
        "the failure must name the directory the check looked for"
    )
    assert "is missing" in observed.stderr, (
        f"the failure must say the directory is missing, not {observed.stderr!r}"
    )


def test_the_linux_check_accepts_the_root_the_chooser_selected(
    tmp_path: pathlib.Path,
) -> None:
    """The real script passes on the environment both passes would leave."""
    environment = _environment(tmp_path)
    expected = pathlib.Path(environment["RUNNER_TEMP"]) / DERIVED_DIRECTORY
    expected.mkdir(parents=True)
    supplied = {
        "CARGO_TARGET_DIR": str(expected),
        "CARGO_LLVM_COV_TARGET_DIR": str(expected),
    }

    observed = _completed(
        ["bash", "-c"], _script(LINUX_VERIFY_STEP), {**environment, **supplied}
    )

    assert observed.returncode == 0, observed.stderr
    assert OBSERVED_LINE in observed.stdout
    assert str(expected) in observed.stdout, (
        "the passing step must report the directory it observed"
    )
    summary = pathlib.Path(environment["GITHUB_STEP_SUMMARY"]).read_text(
        encoding="utf-8"
    )
    assert str(expected) in summary, (
        "the observed root must reach the job summary, where a failing run is read"
    )


@pytest.mark.parametrize(
    ("variable", "value"),
    [
        pytest.param("CARGO_TARGET_DIR", "/somewhere/else", id="wrong-path"),
        pytest.param("CARGO_TARGET_DIR", "", id="empty-value"),
        pytest.param("CARGO_LLVM_COV_TARGET_DIR", "/somewhere/else", id="llvm-wrong"),
        pytest.param("CARGO_LLVM_COV_TARGET_DIR", "", id="llvm-empty"),
    ],
)
def test_the_linux_check_rejects_either_variable_going_astray(
    tmp_path: pathlib.Path, variable: str, value: str
) -> None:
    """Each variable is checked, and an empty value counts as set but wrong.

    ``cargo-llvm-cov`` rejects an empty target directory rather than reading it
    as unset, so the check may not treat ``''`` as if the variable were absent.
    """
    environment = _environment(tmp_path)
    expected = pathlib.Path(environment["RUNNER_TEMP"]) / DERIVED_DIRECTORY
    expected.mkdir(parents=True)
    supplied = {
        "CARGO_TARGET_DIR": str(expected),
        "CARGO_LLVM_COV_TARGET_DIR": str(expected),
        variable: value,
    }

    observed = _completed(
        ["bash", "-c"], _script(LINUX_VERIFY_STEP), {**environment, **supplied}
    )

    assert observed.returncode == 1, (
        f"{variable}={value!r} must fail the lane"
    )
    assert variable in observed.stderr, (
        f"the failure must name {variable}, not report {observed.stderr!r}"
    )


def test_the_linux_check_requires_the_chosen_directory(
    tmp_path: pathlib.Path,
) -> None:
    """Matching variables do not pass the check: the directory must exist."""
    environment = _environment(tmp_path)
    expected = pathlib.Path(environment["RUNNER_TEMP"]) / DERIVED_DIRECTORY
    supplied = {
        "CARGO_TARGET_DIR": str(expected),
        "CARGO_LLVM_COV_TARGET_DIR": str(expected),
    }

    observed = _completed(
        ["bash", "-c"], _script(LINUX_VERIFY_STEP), {**environment, **supplied}
    )

    assert observed.returncode == 1, "an absent chosen root must fail the lane"
    assert str(expected) in observed.stderr
    assert "is missing" in observed.stderr, (
        f"the failure must say the directory is missing, not {observed.stderr!r}"
    )