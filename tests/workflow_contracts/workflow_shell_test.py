"""Contracts for reading shell Make invocations in workflow scripts.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest
from workflow_shell import make_goals


@pytest.mark.parametrize(
    "run",
    [
        "make test 2>&1 | tee log",
        "make test > log",
        "make test >log",
    ],
)
def test_make_goals_ignore_redirection_operators_and_targets(run: str) -> None:
    """Make goals exclude shell redirections and their targets."""
    assert make_goals(run) == [["test"]]
