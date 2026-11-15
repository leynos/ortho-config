"""Bounded properties for reading Make goals from workflow shell fragments.

The oracle builds command groups from known goals and independently renders
their shell syntax. It covers boundaries the repository's own workflows do
not need to use.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import typing as typ

from hypothesis import given, settings
from hypothesis import strategies as st

import workflow_shell

SETTINGS: typ.Final[settings] = settings(
    max_examples=100, derandomize=True, database=None, deadline=None
)

_SAFE_WORD = st.text(alphabet="abcXYZ012", min_size=1, max_size=6)
_SHELL_WORD = st.sampled_from(
    ["&&", "||", ";", "|", "(", ")", "a b", "a#b", "a'b", r"a\b", "a&b"]
)
_ESCAPED_CHARS: typ.Final[frozenset[str]] = frozenset("\\'\";|&()# ")
_REDIRECTION = st.sampled_from(["", " > report.log", "2>&1", " >>report.log"])
_BOUNDARY = st.sampled_from([";", "&&", "||", "|"])


def _single_quoted(value: str) -> str:
    """Render one value as a shell single-quoted word."""
    return "'" + value.replace("'", "'\\''") + "'"


def _backslash_escaped(value: str) -> str:
    """Escape shell-special characters so the shell keeps one word."""
    return "".join(f"\\{char}" if char in _ESCAPED_CHARS else char for char in value)


@st.composite
def _workflow_shell_model(draw: st.DrawFn) -> tuple[str, list[list[str]]]:
    """Render generated Make goals with independently retained expectations."""
    command_count = draw(st.integers(min_value=2, max_value=4))
    commands: list[list[str]] = []
    fragments: list[str] = []
    for _ in range(command_count):
        plain = draw(_SAFE_WORD)
        quoted = draw(_SHELL_WORD)
        escaped = draw(_SHELL_WORD)
        goals = [plain, quoted, escaped]
        commands.append(goals)
        fragments.append(
            "make \\\n "
            + plain
            + " \\\r\n "
            + _single_quoted(quoted)
            + " "
            + _backslash_escaped(escaped)
            + " MODE=${{ matrix.mode }}"
            + draw(_REDIRECTION)
        )
    boundaries = draw(
        st.lists(_BOUNDARY, min_size=command_count - 1, max_size=command_count - 1)
    )
    script = fragments[0]
    for boundary, fragment in zip(boundaries, fragments[1:], strict=True):
        script += f" {boundary} {fragment}"
    script += " # ignored make test\n"
    return script, commands


@SETTINGS
@given(_workflow_shell_model())
def test_make_goals_matches_the_generated_shell_model(
    model: tuple[str, list[list[str]]],
) -> None:
    """Quotes, escapes, continuations and operators preserve the model goals."""
    script, expected = model

    assert workflow_shell.make_goals(script) == expected, f"unexpected goals for {script!r}"
