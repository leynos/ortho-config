"""Two coverage-lane facts about this repository that the shared rules do not know.

The CV-005 rules live in `cv005-contracts` in leynos/shared-actions, run by
`make test-workflow-contracts`. These two were part of the contract this
repository used to carry, and they are repository decisions rather than
rules: which matrix legs have a baseline, and how deep the pull-request
checkout is.
"""

from __future__ import annotations

import pathlib
import typing as typ

from workflow_reading import read_workflows, workflow_jobs

WORKFLOWS: typ.Final[pathlib.Path] = (
    pathlib.Path(__file__).resolve().parents[2] / ".github" / "workflows"
)


def _build_test() -> dict[str, object]:
    """Return the `build-test` job of `ci.yml`."""
    return workflow_jobs(read_workflows(WORKFLOWS)["ci.yml"])["build-test"]


def test_only_the_linux_leg_ratchets() -> None:
    """The ratchet is the gate CV-005 leaves, and only Linux has a baseline.

    generate-coverage keys its baseline by ``runner.os`` and the publisher runs
    on Linux, so a Windows leg that ratcheted would compare against nothing,
    and a Linux leg that did not would stop gating. The matrix's `ratchet` value
    is therefore true exactly for the Linux entry.
    """
    strategy = _build_test()["strategy"]
    include = strategy["matrix"]["include"]  # type: ignore[index]
    ratchets = {row["platform"]: row["ratchet"] for row in include}
    assert ratchets == {"linux": "true", "windows": "false"}, ratchets


def test_the_pull_request_lane_checks_out_shallowly() -> None:
    """``build-test`` takes the default depth-1 checkout.

    Full history was fetched only so ``cs-coverage check`` could diff against
    the merge base, and that gate left the lane with CV-005. generate-coverage
    runs no git command, and the job's other steps read only ``git ls-files``
    and ``git status``, so a full fetch would be a cost with no reader.
    """
    checkouts = [
        step
        for step in _build_test().get("steps", [])  # type: ignore[attr-defined]
        if isinstance(step, dict) and "actions/checkout@" in str(step.get("uses", ""))
    ]
    assert len(checkouts) == 1, f"build-test checks out {len(checkouts)} times"
    inputs = checkouts[0].get("with") or {}
    assert "fetch-depth" not in inputs, (
        f"build-test's checkout sets fetch-depth {inputs.get('fetch-depth')!r}; "
        f"nothing in the job reads history"
    )
