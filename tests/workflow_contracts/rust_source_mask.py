"""Mask Rust comments and literals while retaining source positions.

This reader exists for :mod:`rust_module_graph`, whose inline-module walker
needs to count braces without counting braces inside comments and literals.
It is not a Rust parser; its only permitted caller is that module-graph reader.
"""

from __future__ import annotations

import re

__all__ = ["code_mask"]

_RAW_STRING_START = re.compile(r"(?:br|cr|r)(?P<hashes>\#*)\"")
_CHARACTER = re.compile(r"'(?:\\.|[^'\\\n])'")


def code_mask(text: str) -> str:
    """Blank comments and literals while preserving source offsets.

    Examples
    --------
    A brace inside a string or comment is not a module boundary:

    >>> code_mask('fn f() { let s = "}"; /* } */ }').count("}")
    1
    """
    masked = list(text)
    index = 0
    while index < len(text):
        if text.startswith("//", index):
            end = text.find("\n", index)
            end = len(text) if end < 0 else end
            _blank(masked, index, end)
            index = end
        elif text.startswith("/*", index):
            end = _block_comment_end(text, index)
            _blank(masked, index, end)
            index = end
        else:
            raw_start = _raw_string_start(text, index)
            if raw_start is not None:
                end, delimiter = raw_start
                closing = text.find(delimiter, end)
                end = len(text) if closing < 0 else closing + len(delimiter)
                _blank(masked, index, end)
                index = end
            elif text[index] == '"':
                end = _quoted_end(text, index, '"')
                _blank(masked, index, end)
                index = end
            elif text[index] == "'":
                character = _CHARACTER.match(text, index)
                if character is not None:
                    _blank(masked, index, character.end())
                    index = character.end()
                else:
                    index += 1
            else:
                index += 1
    return "".join(masked)


def _blank(masked: list[str], start: int, end: int) -> None:
    """Replace non-newline characters with spaces, preserving offsets.

    Examples
    --------
    >>> chars = list("a\\nb")
    >>> _blank(chars, 0, len(chars))
    >>> "".join(chars) == " \\n "
    True
    """
    for index in range(start, end):
        if masked[index] != "\n":
            masked[index] = " "


def _block_comment_end(text: str, start: int) -> int:
    """Return the end of a possibly nested Rust block comment.

    Examples
    --------
    >>> _block_comment_end("/* outer /* inner */ end */x", 0)
    27
    """
    depth = 1
    index = start + 2
    while index < len(text) and depth:
        if text.startswith("/*", index):
            depth += 1
            index += 2
        elif text.startswith("*/", index):
            depth -= 1
            index += 2
        else:
            index += 1
    return index


def _raw_string_start(text: str, index: int) -> tuple[int, str] | None:
    """Return raw-string content position and its closing delimiter.

    Examples
    --------
    >>> _raw_string_start('r##"{ text }"##', 0)
    (4, '"##')
    >>> _raw_string_start("identifier", 0) is None
    True
    """
    if index and (text[index - 1].isalnum() or text[index - 1] == "_"):
        return None
    match = _RAW_STRING_START.match(text, index)
    if match is None:
        return None
    hashes = match.group("hashes")
    return match.end(), f'"{hashes}'


def _quoted_end(text: str, start: int, quote: str) -> int:
    """Return a quoted literal's end, including its closing quote.

    Examples
    --------
    >>> _quoted_end(r'"escaped \\" quote" tail', 0, '"')
    18
    """
    index = start + 1
    while index < len(text):
        if text[index] == "\\":
            index += 2
        elif text[index] == quote:
            return index + 1
        else:
            index += 1
    return len(text)
