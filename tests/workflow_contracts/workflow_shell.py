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
_REDIRECTION: typ.Final[re.Pattern[str]] = re.compile(
    r"^(?P<operator>\d*(?:<<<|<<-|<<|<>|<&|>>|>\||>&|<|>)|&(?:>>|>|<))(?P<target>.*)$"
)


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
        """Drop a word-start comment and end the line there.

        The comment is removed here rather than left for ``shlex`` to discard:
        Python 3.14's ``shlex`` treats ``#`` as a comment character even inside
        an unquoted word, so a URL fragment such as
        ``https://example.invalid/#fragment`` would swallow the Make invocation
        that followed it when comments were enabled downstream. Only a ``#``
        that starts a word, outside quotes, is a comment to the shell; every
        other occurrence is data this scanner keeps.
        """
        if self.quote is not None:
            return False
        if self.source[self.index] != "#":
            return False
        if not self.at_word_start:
            return False
        end = self.source.find("\n", self.index)
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

    Comment processing is off because :func:`_ShellLineScanner` has already
    removed the shell's real comments. Leaving it on would let ``shlex`` treat
    a ``#`` inside a word, such as a URL fragment, as the start of a comment
    and discard the rest of the line.
    """
    try:
        return shlex.split(_EXPRESSION.sub("EXPR", line), comments=False)
    except ValueError:
        return []


def _redirected_target(word: str) -> bool | None:
    """Return whether a redirection operator needs its following target."""
    redirection = _REDIRECTION.fullmatch(word)
    return None if redirection is None else not redirection.group("target")


@dataclasses.dataclass(frozen=True)
class MakeInvocation:
    """One Make invocation: the goals it asks for and the variables it sets.

    Attributes
    ----------
    goals : list of str
        The Make goals, in the order written, options and assignments omitted.
    assignments : dict
        The command-line ``NAME=value`` variables, which outrank both the
        environment and the Makefile itself.
    """

    goals: list[str]
    assignments: dict[str, str]


def _invocation_after(tokens: list[str], index: int) -> MakeInvocation:
    """Return the goals and assignments following a ``make`` word."""
    goals: list[str] = []
    assignments: dict[str, str] = {}
    skip_target = False
    for word in tokens[index + 1 :]:
        if word == _SHELL_CONTROL_MARKER:
            break
        if skip_target:
            skip_target = False
            continue
        target_follows = _redirected_target(word)
        if target_follows is not None:
            skip_target = target_follows
            continue
        if word.startswith("-"):
            continue
        name, separator, value = word.partition("=")
        if not separator:
            goals.append(word)
        elif name:
            assignments[name] = value
    return MakeInvocation(goals, assignments)


def make_invocations(run: str) -> list[MakeInvocation]:
    """Return the Make invocations in a GitHub Actions script, in order.

    Parameters
    ----------
    run : str
        The GitHub Actions ``run`` script to inspect.

    Returns
    -------
    list of MakeInvocation
        One per ``make`` word, with the goals and command-line assignments that
        follow it up to the next shell command boundary.
    """
    source = _EXPRESSION.sub("EXPR", run)
    tokenised = (_tokens(line) for line in _logical_lines(source))
    return [
        _invocation_after(tokens, index)
        for tokens in tokenised
        for index, token in enumerate(tokens)
        if token == "make"
    ]


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
    return [invocation.goals for invocation in make_invocations(run)]
