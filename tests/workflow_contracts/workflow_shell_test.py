"""Contracts for reading shell Make invocations in workflow scripts.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest
from workflow_shell import MakeInvocation, make_goals, make_invocations


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


def test_command_line_assignments_are_read_beside_the_goals() -> None:
    """A ``NAME=value`` word sets a Make variable, and is not a goal."""
    (invocation,) = make_invocations("make lint BUILD_STANDARD=0")

    assert invocation.goals == ["lint"]
    assert invocation.assignments == {"BUILD_STANDARD": "0"}


def test_assignments_belong_to_the_invocation_that_carries_them() -> None:
    """A later command's assignment does not leak back to an earlier one."""
    first, second = make_invocations("make lint && make test BUILD_STANDARD=0")

    assert first == MakeInvocation(["lint"], {})
    assert second == MakeInvocation(["test"], {"BUILD_STANDARD": "0"})


def test_embedded_hash_keeps_the_make_invocation_after_it() -> None:
    """A ``#`` inside an unquoted word is data, not the start of a comment.

    Python 3.14's ``shlex`` ends the line at a ``#`` inside a word when comment
    processing is on, which would hide this ``make`` from the provisioning
    contract.
    """
    run = "echo https://example.invalid/#fragment; make check-build-tools"

    assert make_goals(run) == [["check-build-tools"]]


def test_word_start_comment_is_dropped_without_losing_the_command() -> None:
    """A genuine comment ends at its line, leaving the command intact."""
    run = "make test # run the suite\nmake check-build-tools"

    assert make_goals(run) == [["test"], ["check-build-tools"]]


def test_quoted_hash_survives_tokenisation() -> None:
    """A ``#`` inside quotes is ordinary text for the shell."""
    run = "make test MODE='#fragment' && make check-build-tools"

    assert make_goals(run) == [["test"], ["check-build-tools"]]
