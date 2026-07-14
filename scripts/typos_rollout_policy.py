"""Validate shared spelling authorities and narrow local exceptions.

Local regexes must not match empty or generic prose, and local file globs must
not exclude every Markdown path.
"""

from __future__ import annotations

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
UNIVERSAL_FILE_GLOBS = frozenset({"*", "**", "**/*", "*.md", "**/*.md"})


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
    """Validate schema identity and required shared-authority fields."""
    schema = document.get("schema")
    if not _has_valid_schema(schema):
        message = f"unsupported dictionary schema {schema!r}"
        raise ValueError(message)
    if sparse:
        return
    for table_name, field_name in REQUIRED_AUTHORITY_FIELDS:
        _validate_required_authority_field(document, table_name, field_name)


def _is_broad_ignore_pattern(pattern: str) -> bool:
    """Return whether an ignore regex can match generic repository prose."""
    compiled = re.compile(pattern)
    return compiled.search("") is not None or any(
        compiled.fullmatch(probe) for probe in GENERIC_PROSE
    )


def _is_broad_file_exclusion(pattern: str) -> bool:
    """Return whether a file glob excludes all repository Markdown."""
    return pattern in UNIVERSAL_FILE_GLOBS


def validate_local_exceptions(
    ignore_patterns: tuple[str, ...],
    excluded_files: tuple[str, ...],
) -> None:
    """Reject local exceptions that can match generic prose or all Markdown."""
    for pattern in filter(_is_broad_ignore_pattern, ignore_patterns):
        message = f"local ignore pattern is too broad: {pattern!r}"
        raise ValueError(message)
    for pattern in filter(_is_broad_file_exclusion, excluded_files):
        message = f"local file exclusion is too broad: {pattern!r}"
        raise ValueError(message)
