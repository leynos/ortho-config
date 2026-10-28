"""Read just enough shell syntax to find Make goals in workflow ``run`` steps.

This is a deliberately narrow workflow-contract helper, not a general shell
parser. It folds line continuations and marks unquoted command boundaries so
the provisioning contract can identify Make invocations conservatively.
"""

from __future__ import annotations

import dataclasses
import re
import shlex
import typing as typ

_EXPRESSION: typ.Final[re.Pattern[str]] = re.compile(r"\$\{\{.*?\}\}")
_SHELL_CONTROL_MARKER: typ.Final[str] = "\ufdd0"
_SHELL_CONTROL_CHARS: typ.Final[str] = ";|&()"


@dataclasses.dataclass
class _ShellLineScanner:
    """Fold physical lines and mark command boundaries outside quotes."""

    source: str
    lines: list[str] = dataclasses.field(default_factory=list)
    current: list[str] = dataclasses.field(default_factory=list)
    quote: str | None = None
    at_word_start: bool = True
    index: int = 0

    def read(self) -> list[str]:
        """Return logical lines with shell command boundaries marked."""
        while self.index < len(self.source):
            if self._consume_escape():
                continue
            if self._consume_comment():
                continue
            if self._consume_operator():
                continue
            self._consume_character()
        if self.current:
            self._finish_line()
        return self.lines

    def _consume_escape(self) -> bool:
        """Consume an escaped character or remove an unquoted line continuation."""
        if self.source[self.index] != "\\":
            return False
        if self.quote == "'":
            return False
        if self.index + 1 >= len(self.source):
            return False
        newline_length = self._continuation_length()
        if newline_length:
            self.index += 1 + newline_length
            return True
        self.current.extend(("\\", self.source[self.index + 1]))
        self.at_word_start = False
        self.index += 2
        return True

    def _continuation_length(self) -> int:
        """Return the width of the newline after the current backslash."""
        following = self.index + 1
        if self.source.startswith("\r\n", following):
            return 2
        return int(self.source[following] == "\n")

    def _consume_comment(self) -> bool:
        """Finish a word-start comment, retaining it for ``shlex`` to discard."""
        if self.quote is not None:
            return False
        if self.source[self.index] != "#":
            return False
        if not self.at_word_start:
            return False
        end = self.source.find("\n", self.index)
        comment = self.source[self.index :] if end == -1 else self.source[self.index : end]
        self.current.extend(comment)
        if end == -1:
            self.index = len(self.source)
        else:
            self.index = end + 1
            self._finish_line()
        return True

    def _consume_operator(self) -> bool:
        """Replace a run of unquoted shell control characters with a marker."""
        char = self.source[self.index]
        if self.quote is not None or char not in _SHELL_CONTROL_CHARS:
            return False
        while self.index < len(self.source) and self.source[self.index] in _SHELL_CONTROL_CHARS:
            self.index += 1
        self.current.extend((" ", _SHELL_CONTROL_MARKER, " "))
        self.at_word_start = True
        return True

    def _consume_character(self) -> None:
        """Append one ordinary character and finish unquoted physical lines."""
        char = self.source[self.index]
        if self.quote is None and char in {'"', "'"}:
            self.quote = char
        elif self.quote == char:
            self.quote = None
        if char == "\n" and self.quote is None:
            self._finish_line()
            self.index += 1
            return
        self.current.append(char)
        self.at_word_start = char.isspace() or char in _SHELL_CONTROL_CHARS
        self.index += 1

    def _finish_line(self) -> None:
        """Append the buffered line and reset word-boundary state."""
        self.lines.append("".join(self.current))
        self.current.clear()
        self.at_word_start = True


def _logical_lines(run: str) -> list[str]:
    """Return shell logical lines while preserving quoted operators and comments."""
    return _ShellLineScanner(run).read()


def _tokens(line: str) -> list[str]:
    """Split one line into words, returning none when the shell cannot parse it.

    GitHub resolves expressions such as ``MSRV=${{ matrix.toolchain }}``
    before invoking the shell. Replacing each expression keeps its embedded
    spaces from looking like extra goals to this conservative reader.
    """
    try:
        return shlex.split(_EXPRESSION.sub("EXPR", line), comments=True)
    except ValueError:
        return []


def _goals_after(tokens: list[str], index: int) -> list[str]:
    """Return Make goals until the next command boundary, omitting options."""
    goals: list[str] = []
    for word in tokens[index + 1 :]:
        if word == _SHELL_CONTROL_MARKER:
            break
        if not word.startswith("-") and "=" not in word:
            goals.append(word)
    return goals


def make_goals(run: str) -> list[list[str]]:
    """Return goal lists for Make invocations in a GitHub Actions script.

    Parameters
    ----------
    run : str
        The GitHub Actions ``run`` script to inspect.

    Returns
    -------
    list[list[str]]
        The goals for each Make invocation, in script order.
    """
    source = _EXPRESSION.sub("EXPR", run)
    tokenised = (_tokens(line) for line in _logical_lines(source))
    return [
        _goals_after(tokens, index)
        for tokens in tokenised
        for index, token in enumerate(tokens)
        if token == "make"
    ]
