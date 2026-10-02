"""Contract for the ``spelling`` alias of the spelling gate.

The shared ``AGENTS.md`` spelling block tells agents to run ``make spelling``,
but this repository's gate target is ``spellcheck``, which ``make
markdownlint`` and CI run. The ``spelling`` target exists only so that the
command the block names is real. These tests run it: ``make spelling`` is
invoked with ``TYPOS_CONFIG_BUILDER`` overridden by a fake builder that records
its arguments, so the contract proves what the alias executes, how a verdict
propagates, and that ``spellcheck`` and ``spelling`` reach the same command.
Reading the Makefile text would accept a comment or an ``echo`` in place of the
gate.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import shutil
import stat
import subprocess  # noqa: S404 - the contract runs make against a fake builder
import typing as typ
from pathlib import Path

import pytest

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

#: The alias and the target it must reach.
ALIAS: typ.Final[str] = "spelling"
GATE_TARGET: typ.Final[str] = "spellcheck"

#: The arguments the gate must receive.
GATE_ARGUMENTS: typ.Final[list[str]] = ["gate", "--repository", "."]


def _fake_builder(directory: Path, exit_status: int) -> tuple[Path, Path]:
    """Write a builder stub that records its arguments and exits.

    Parameters
    ----------
    directory : Path
        Where the stub and its argument log are created.
    exit_status : int
        The status the stub exits with.

    Returns
    -------
    tuple of Path
        The stub's path and the file its arguments are appended to.
    """
    log = directory / "arguments.log"
    stub = directory / "fake-builder"
    stub.write_text(
        f'#!/bin/sh\nprintf "%s\\n" "$*" >> "{log}"\nexit {exit_status}\n',
        encoding="utf-8",
    )
    stub.chmod(stub.stat().st_mode | stat.S_IXUSR)
    return stub, log


def _run_make(target: str, stub: Path) -> subprocess.CompletedProcess[str]:
    """Run one make target with the builder replaced by ``stub``."""
    make = shutil.which("make")
    assert make is not None, "make must be available to run the contract"
    return subprocess.run(  # noqa: S603 - fixed argument vector, no shell
        [make, "-C", str(REPOSITORY_ROOT), target, f"TYPOS_CONFIG_BUILDER={stub}"],
        capture_output=True,
        text=True,
        check=False,
    )


@pytest.mark.parametrize("target", [ALIAS, GATE_TARGET])
def test_target_runs_the_builder_gate_once(target: str, tmp_path: Path) -> None:
    """Each target executes ``gate --repository .`` exactly once and succeeds.

    Scenario: the AGENTS.md block names ``make spelling``. Invariant: that
    target and the gate target both execute the builder with the gate
    arguments, so a comment or an ``echo`` in their place is rejected.
    """
    stub, log = _fake_builder(tmp_path, exit_status=0)
    result = _run_make(target, stub)
    assert result.returncode == 0, result.stderr
    calls = log.read_text(encoding="utf-8").splitlines()
    assert calls == [" ".join(GATE_ARGUMENTS)], (
        f"`make {target}` must run the builder once with {GATE_ARGUMENTS}; saw {calls}"
    )


@pytest.mark.parametrize("target", [ALIAS, GATE_TARGET])
def test_target_fails_when_the_builder_fails(target: str, tmp_path: Path) -> None:
    """A failing builder fails the target, so a finding cannot pass silently.

    Scenario: the gate reports a misspelling and exits non-zero. Invariant:
    ``make spelling`` and ``make spellcheck`` both propagate the failure rather
    than masking it.
    """
    stub, log = _fake_builder(tmp_path, exit_status=3)
    result = _run_make(target, stub)
    assert result.returncode != 0, f"`make {target}` must fail when the gate fails"
    assert log.read_text(encoding="utf-8").splitlines() == [" ".join(GATE_ARGUMENTS)]
