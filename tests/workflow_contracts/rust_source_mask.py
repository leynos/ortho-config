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
        index = _mask_construct(text, masked, index)
    return "".join(masked)


def _mask_construct(text: str, masked: list[str], index: int) -> int:
    """Mask whatever starts at ``index`` and return the next unread position.

    Examples
    --------
    >>> chars = list('"}" x')
    >>> _mask_construct('"}" x', chars, 0)
    3
    >>> "".join(chars)
    '    x'
    """
    if text.startswith("//", index):
        return _mask_until(masked, index, _line_comment_end(text, index))
    if text.startswith("/*", index):
        return _mask_until(masked, index, _block_comment_end(text, index))
    raw = _raw_string_start(text, index)
    if raw is not None:
        return _mask_until(masked, index, _raw_string_end(text, raw))
    if text[index] == '"':
        return _mask_until(masked, index, _quoted_end(text, index, '"'))
    if text[index] == "'":
        return _mask_character(text, masked, index)
    return index + 1


def _mask_until(masked: list[str], start: int, end: int) -> int:
    """Blank one construct and report where the next scan resumes."""
    _blank(masked, start, end)
    return end


def _mask_character(text: str, masked: list[str], index: int) -> int:
    """Mask a character literal, or step over a lone apostrophe.

    Examples
    --------
    >>> chars = list("'}' x")
    >>> _mask_character("'}' x", chars, 0)
    3
    >>> "".join(chars)
    '    x'
    """
    character = _CHARACTER.match(text, index)
    if character is None:
        return index + 1
    return _mask_until(masked, index, character.end())


def _line_comment_end(text: str, index: int) -> int:
    """Return the end of a line comment, excluding its newline."""
    end = text.find("\n", index)
    return len(text) if end < 0 else end


def _raw_string_end(text: str, raw: tuple[int, str]) -> int:
    """Return the end of a raw string, given its content start and delimiter."""
    content_start, delimiter = raw
    closing = text.find(delimiter, content_start)
    return len(text) if closing < 0 else closing + len(delimiter)


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


def _is_identifier_character(character: str) -> bool:
    """Report whether a character can continue a Rust identifier.

    A raw-string prefix glued to an identifier, as in ``myr"x"``, is part of
    the name rather than a literal opening.

    Examples
    --------
    >>> _is_identifier_character("r"), _is_identifier_character("_")
    (True, True)
    >>> _is_identifier_character(" ")
    False
    """
    return character.isalnum() or character == "_"


def _raw_string_start(text: str, index: int) -> tuple[int, str] | None:
    """Return raw-string content position and its closing delimiter.

    Examples
    --------
    >>> _raw_string_start('r##"{ text }"##', 0)
    (4, '"##')
    >>> _raw_string_start("identifier", 0) is None
    True
    """
    if index and _is_identifier_character(text[index - 1]):
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
