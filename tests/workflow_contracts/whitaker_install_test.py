"""Contract test: Whitaker is installed only through the shared action.

This repository once carried a hand-rolled install step, and a contract
asserting its authentication and its refusal to let binstall compile
`dylint-link`. The shared `install-whitaker` action now owns the install: it
downloads a digest-verified release archive, passes `--no-source-fallback`,
and takes `github.token` itself. Its own contract proves those properties, so
this one asserts only what stays the repository's business: the Linux lint leg
uses the action at a full commit SHA and carries no script of its own, so the
hand-rolled route cannot return unnoticed.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import yaml

WORKFLOW_PATH: typ.Final[Path] = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"
)
STEP_NAME: typ.Final[str] = "Install Whitaker"
ACTION_PIN: typ.Final[re.Pattern[str]] = re.compile(
    r"leynos/shared-actions/\.github/actions/install-whitaker@[0-9a-f]{40}"
)
LINUX_ONLY: typ.Final[str] = "${{ matrix.platform == 'linux' }}"


def _install_steps() -> list[dict[str, typ.Any]]:
    """Return every step named like the Whitaker install, across all jobs."""
    workflow = yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))
    return [
        step
        for job in workflow["jobs"].values()
        for step in job.get("steps", [])
        if step.get("name") == STEP_NAME
    ]


def test_whitaker_is_installed_through_the_pinned_shared_action() -> None:
    """Exactly one step installs Whitaker, through the action at a full SHA.

    Invariant: the step has a `uses` matching the action at a 40-hex commit,
    and no `run` script. The count is asserted first, because a contract over
    no steps is satisfied by deleting the install.
    """
    steps = _install_steps()
    assert len(steps) == 1, f"expected one {STEP_NAME!r} step, found {len(steps)}"
    (step,) = steps
    assert ACTION_PIN.fullmatch(str(step.get("uses", ""))), (
        f"{STEP_NAME!r} must use the shared action at a full commit SHA"
    )
    assert "run" not in step, f"{STEP_NAME!r} must not carry a script of its own"


def test_the_install_runs_on_the_linux_leg_only() -> None:
    """Whitaker lints on Linux; the Windows leg runs Clippy alone."""
    (step,) = _install_steps()
    assert step.get("if") == LINUX_ONLY, (
        f"{STEP_NAME!r} must stay guarded to the Linux leg"
    )
