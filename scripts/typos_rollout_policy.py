"""Validate shared spelling authorities and narrow local exceptions.

Local regexes must not match empty or generic prose, and local file globs must
not exclude every Markdown path.
"""

from __future__ import annotations

import dataclasses as dc
import re
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

SCHEMA_VERSION = 1
REQUIRED_AUTHORITY_FIELDS = (
    ("oxford", "stems"),
    ("words", "accepted"),
    ("words", "corrections"),
    ("phrases", "corrections"),
    ("patterns", "ignore"),
    ("files", "exclude"),
)
GENERIC_PROSE = ("ordinary prose", "unrelated_identifier")
UNIVERSAL_FILE_GLOBS = frozenset({"*", "**", "**/*", "*.md", "**.md", "**/*.md"})
BACKREFERENCE = re.compile(r"\\(?:[1-9]|g<|k<)|\(\?P=")
REPETITION = re.compile(r"\{\d+(?:,\d*)?\}")


@dc.dataclass(slots=True)
class _GroupState:
    """Track ambiguity and adjacent quantified atoms within one regex group."""

    has_repetition: bool = False
    has_alternation: bool = False
    atoms_since_repetition: int | None = None

    def note_atom(self) -> None:
        """Record one atom separating this group's direct repetitions."""
        if self.atoms_since_repetition is not None:
            self.atoms_since_repetition += 1


def _has_valid_schema(schema: object) -> bool:
    """Return whether a schema value identifies the supported policy format."""
    return (
        isinstance(schema, int)
        and not isinstance(schema, bool)
        and schema == SCHEMA_VERSION
    )


def _validate_required_authority_field(
    document: cabc.Mapping[str, object],
    table_name: str,
    field_name: str,
) -> None:
    """Require one table and field from a complete shared authority."""
    if table_name not in document:
        message = f"missing required table {table_name!r}"
        raise ValueError(message)
    table = document[table_name]
    if isinstance(table, dict) and field_name not in table:
        message = f"missing required field {table_name}.{field_name}"
        raise ValueError(message)


def validate_document(
    document: cabc.Mapping[str, object],
    *,
    sparse: bool,
) -> None:
    """Validate schema identity and required shared-authority fields.

    Parameters
    ----------
    document
        Parsed TOML document to validate.
    sparse
        Whether to permit a local overlay that omits shared-policy fields.

    Raises
    ------
    ValueError
        If the schema is unsupported, or a complete authority omits a required
        table or field.
    """
    schema = document.get("schema")
    if not _has_valid_schema(schema):
        message = f"unsupported dictionary schema {schema!r}"
        raise ValueError(message)
    if sparse:
        return
    for table_name, field_name in REQUIRED_AUTHORITY_FIELDS:
        _validate_required_authority_field(document, table_name, field_name)


def _consume_repetition(
    pattern: str,
    position: int,
    group: _GroupState,
    previous_group: tuple[bool, bool],
) -> tuple[int, bool, bool]:
    """Consume one repetition suffix and report its syntax and safety."""
    previous_was_group, previous_group_is_ambiguous = previous_group
    character = pattern[position]
    repetition = REPETITION.match(pattern, position)
    is_suffix = character in "*+?" or repetition is not None
    is_group_syntax = character == "?" and pattern[position - 1 : position] == "("
    is_modifier = character in "+?" and pattern[position - 1 : position] in "*+?}"
    if not is_suffix or is_group_syntax or is_modifier:
        return position, False, is_group_syntax or is_modifier
    is_unsafe = group.atoms_since_repetition == 1 or (
        previous_was_group and previous_group_is_ambiguous
    )
    group.has_repetition = True
    group.atoms_since_repetition = 0
    end = position if repetition is None else repetition.end() - 1
    return end, is_unsafe, True


def _has_unsafe_repetition(pattern: str) -> bool:
    """Return whether repetition can amplify another ambiguous expression."""
    groups = [_GroupState()]
    previous_was_group = False
    previous_group_is_ambiguous = False
    in_character_class = False
    position = 0
    while position < len(pattern):
        character = pattern[position]
        if in_character_class:
            if character == "\\":
                position += 2
                continue
            in_character_class = character != "]"
            position += 1
            continue
        if character == "\\":
            groups[-1].note_atom()
            position += 2
            previous_was_group = False
            continue
        if character == "[":
            in_character_class = True
            groups[-1].note_atom()
            previous_was_group = False
        elif character == "(":
            groups.append(_GroupState())
            previous_was_group = False
        elif character == ")" and len(groups) > 1:
            closed_group = groups.pop()
            groups[-1].note_atom()
            groups[-1].has_repetition |= closed_group.has_repetition
            groups[-1].has_alternation |= closed_group.has_alternation
            previous_was_group = True
            previous_group_is_ambiguous = (
                closed_group.has_repetition or closed_group.has_alternation
            )
        elif character == "|":
            groups[-1].has_alternation = True
            groups[-1].atoms_since_repetition = None
            previous_was_group = False
        else:
            position, is_unsafe, is_operator = _consume_repetition(
                pattern,
                position,
                groups[-1],
                (previous_was_group, previous_group_is_ambiguous),
            )
            if is_unsafe:
                return True
            if not is_operator:
                groups[-1].note_atom()
            previous_was_group = False
        position += 1
    return False


def _compile_policy_pattern(pattern: str) -> re.Pattern[str]:
    """Compile a policy regex after rejecting backtracking-prone forms."""
    try:
        compiled = re.compile(pattern)
    except re.error as error:
        message = f"ignore pattern is invalid: {pattern!r} ({error})"
        raise ValueError(message) from error
    if BACKREFERENCE.search(pattern) or _has_unsafe_repetition(pattern):
        message = f"ignore pattern has unsafe repetition: {pattern!r}"
        raise ValueError(message)
    return compiled


def compile_ignore_patterns(
    ignore_patterns: tuple[str, ...],
) -> tuple[re.Pattern[str], ...]:
    """Compile policy regexes with bounded matching complexity.

    Parameters
    ----------
    ignore_patterns
        Regular expressions used to mask quoted or upstream-controlled text.

    Returns
    -------
    tuple[re.Pattern[str], ...]
        Compiled patterns in their policy order.

    Raises
    ------
    ValueError
        If a pattern is malformed or contains backtracking-prone repetition.
    """
    return tuple(_compile_policy_pattern(pattern) for pattern in ignore_patterns)


def validate_ignore_patterns(ignore_patterns: tuple[str, ...]) -> None:
    """Validate that policy regexes have bounded matching complexity.

    Parameters
    ----------
    ignore_patterns
        Regular expressions used to mask quoted or upstream-controlled text.

    Raises
    ------
    ValueError
        If a pattern is malformed or contains backtracking-prone repetition.
    """
    compile_ignore_patterns(ignore_patterns)


def _is_broad_ignore_pattern(pattern: str) -> bool:
    """Return whether an ignore regex can match generic repository prose."""
    compiled = _compile_policy_pattern(pattern)
    return compiled.search("") is not None or any(
        compiled.search(probe) for probe in GENERIC_PROSE
    )


def _is_broad_file_exclusion(pattern: str) -> bool:
    """Return whether a file glob excludes all repository Markdown."""
    normalized = pattern.strip().casefold()
    while normalized.startswith("./"):
        normalized = normalized[2:]
    return normalized in UNIVERSAL_FILE_GLOBS


def validate_local_exceptions(
    ignore_patterns: tuple[str, ...],
    excluded_files: tuple[str, ...],
) -> None:
    """Reject local exceptions that weaken shared spelling policy.

    Parameters
    ----------
    ignore_patterns
        Repository-specific regular expressions proposed for masked text.
    excluded_files
        Repository-specific file globs proposed for exclusion.

    Raises
    ------
    ValueError
        If a regex is invalid, unsafe or broad enough to match generic prose,
        or a file glob excludes all repository Markdown.
    """
    for pattern in filter(_is_broad_ignore_pattern, ignore_patterns):
        message = f"local ignore pattern is too broad: {pattern!r}"
        raise ValueError(message)
    for pattern in filter(_is_broad_file_exclusion, excluded_files):
        message = f"local file exclusion is too broad: {pattern!r}"
        raise ValueError(message)
