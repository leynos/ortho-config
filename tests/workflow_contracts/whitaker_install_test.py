"""Contract test: Whitaker is installed only through the shared action.

This repository once carried a hand-rolled install step, and a contract
asserting its authentication and its refusal to let binstall compile
`dylint-link`. The shared `install-whitaker` action now owns the install: it
downloads a digest-verified release archive, passes `--no-source-fallback`,
and takes `github.token` itself. Its own contract proves those properties, so
this one asserts only what stays the repository's business: every job that
lints with Whitaker installs it through the action at a full commit SHA and
carries no install script of its own, so the hand-rolled route cannot return
unnoticed.

Two jobs now install it: the Linux leg of `build-test`, which runs the host
pass, and the Windows-target gate, which scores the `cfg` surface that leg
cannot see. Both are Linux; the Windows continuous-integration leg still runs
Clippy alone.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import yaml
from workflow_legs import legs, runs_on_linux, step_runs_on

WORKFLOW_PATH: typ.Final[Path] = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"
)
STEP_NAME: typ.Final[str] = "Install Whitaker"
ACTION_PIN: typ.Final[re.Pattern[str]] = re.compile(
    r"leynos/shared-actions/\.github/actions/install-whitaker@[0-9a-f]{40}"
)
LINUX_ONLY: typ.Final[str] = "${{ matrix.platform == 'linux' }}"


def _jobs() -> dict[str, dict[str, typ.Any]]:
    """Return the workflow's jobs by name."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    return {
        str(name): job
        for name, job in workflow["jobs"].items()
        if isinstance(job, dict)
    }


def _install_steps() -> list[tuple[str, dict[str, typ.Any]]]:
    """Return every step named like the Whitaker install, with its job name."""
    return [
        (name, step)
        for name, job in _jobs().items()
        for step in job.get("steps", [])
        if isinstance(step, dict) and step.get("name") == STEP_NAME
    ]


def test_whitaker_is_installed_through_the_pinned_shared_action() -> None:
    """Every install step uses the action at a full SHA, and runs no script.

    The count is asserted first, because a contract over no steps is satisfied
    by deleting the install. Two are expected today; the assertion is a floor
    so a third Linux lint job does not need this contract edited, while a lost
    install still fails.
    """
    steps = _install_steps()
    assert len(steps) >= 2, f"expected at least two {STEP_NAME!r} steps, found {len(steps)}"
    for job_name, step in steps:
        assert ACTION_PIN.fullmatch(str(step.get("uses", ""))), (
            f"{STEP_NAME!r} in {job_name} must use the shared action at a full commit SHA"
        )
        assert "run" not in step, (
            f"{STEP_NAME!r} in {job_name} must not carry a script of its own"
        )


def test_every_install_lands_on_a_linux_runner() -> None:
    """Whitaker lints on Linux; the Windows leg runs Clippy alone.

    A step's own guard decides which matrix leg it runs on, and a job without a
    matrix decides with its `runs-on`. Both are evaluated per leg, so a step
    guarded only by a condition that no Windows leg satisfies still fails here
    if its job can land on Windows.
    """
    for job_name, step in _install_steps():
        job = _jobs()[job_name]
        for leg in legs(WORKFLOW_PATH.name, job_name, job):
            if not step_runs_on(step, leg.cell):
                continue
            assert runs_on_linux(leg), (
                f"{STEP_NAME!r} in {job_name} can run on {leg.cell or leg.job.get('runs-on')!r}"
            )


def test_the_build_test_install_stays_guarded_to_its_linux_leg() -> None:
    """`build-test` is a two-platform matrix, so its install needs the guard.

    Dropping the `if` would install Whitaker on the Windows leg, where the
    suite is not staged and the step would fail.
    """
    guarded = [
        step
        for job_name, step in _install_steps()
        if job_name == "build-test"
    ]
    (step,) = guarded
    assert step.get("if") == LINUX_ONLY, (
        f"{STEP_NAME!r} must stay guarded to the Linux leg"
    )
