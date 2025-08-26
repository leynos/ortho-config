#!/usr/bin/env -S uv run
# /// script
# dependencies = ["tomlkit==0.13.*", "markdown-it-py>=3,<4"]
# ///
"""Synchronise workspace and crate versions.

This tool updates the top-level workspace version and each member crate's
version to the supplied value, keeping documentation snippets in sync. It
exists to reduce the risk of publishing mismatched versions across the
workspace.

Examples
--------
Run with the desired semantic version:
    ./scripts/bump_version.py 1.2.3
"""
from __future__ import annotations

import os
import sys
import tempfile
from collections.abc import Mapping, MutableMapping
from pathlib import Path
from typing import Any, Callable

import tomlkit
from markdown_it import MarkdownIt
from tomlkit.exceptions import TOMLKitError


def replace_fences(md_text: str, lang: str, replace_fn: Callable[[str], str]) -> str:
    """Apply ``replace_fn`` to fenced code blocks of ``lang`` in Markdown text.

    Parameters
    ----------
    md_text
        Markdown content to process.
    lang
        Fence language (for example ``"toml"``).
    replace_fn
        Callback invoked with the fence body; its return value replaces the
        original body.

    Examples
    --------
    >>> md = '```toml\n[dependencies]\nfoo = "1"\n```'
    >>> replace_fences(md, 'toml', lambda body: body.replace('1', '2'))
    '```toml\n[dependencies]\nfoo = "2"\n```\n'
    """
    md = MarkdownIt("commonmark")
    tokens = md.parse(md_text)
    lines = md_text.splitlines(keepends=True)
    out: list[str] = []
    last = 0
    for tok in tokens:
        if tok.type != "fence":
            continue
        info_lang = ((tok.info or "").split()[0] or "").lower()
        if info_lang != lang.lower():
            continue
        start, end = tok.map
        out.append("".join(lines[last:start]))
        fence_marker = tok.markup or "```"
        info = tok.info or lang
        new_body = replace_fn(tok.content)
        out.append(f"{fence_marker}{info}\n{new_body}\n{fence_marker}\n")
        last = end
    out.append("".join(lines[last:]))
    return "".join(out)


def _update_package_version(
    doc: MutableMapping[str, object],
    version: str,
) -> None:
    """Update package version in ``doc`` if present.

    Examples
    --------
    >>> data = {"package": {"version": "0"}}
    >>> _update_package_version(data, "1")
    >>> data["package"]["version"]
    '1'
    """
    if "workspace" in doc and "package" in doc["workspace"]:
        doc["workspace"]["package"]["version"] = version
    elif "package" in doc:
        doc["package"]["version"] = version


def _extract_version_prefix(
    entry: tomlkit.items.String | Mapping[str, object] | str | None,
) -> str:
    """Return version prefix (``^`` or ``~``) if present.

    Examples
    --------
    >>> import tomlkit
    >>> doc = tomlkit.parse('foo = "^1"')
    >>> _extract_version_prefix(doc["foo"])
    '^'
    >>> _extract_version_prefix("1")
    ''
    """
    if isinstance(entry, Mapping):
        entry = entry.get("version")
    text = entry.value if isinstance(entry, tomlkit.items.String) else str(entry or "")
    return text[0] if text and text[0] in "^~" else ""


def _update_dict_dependency(
    entry: MutableMapping[str, object],
    version: str,
) -> None:
    """Update dict-style dependency ``entry`` with ``version``.

    Examples
    --------
    >>> import tomlkit
    >>> entry = tomlkit.table()
    >>> entry["version"] = tomlkit.string("^0.1")
    >>> _update_dict_dependency(entry, "1.2.3")
    >>> entry["version"].value
    '^1.2.3'
    """
    prefix = _extract_version_prefix(entry)
    existing = entry.get("version")
    if isinstance(existing, tomlkit.items.String):
        new = tomlkit.string(prefix + version)
        new.trivia.indent = existing.trivia.indent
        new.trivia.comment_ws = existing.trivia.comment_ws
        new.trivia.comment = existing.trivia.comment
        new.trivia.trail = existing.trivia.trail
        entry["version"] = new
    else:
        entry["version"] = prefix + version


def _update_string_dependency(
    deps: MutableMapping[str, object],
    dependency: str,
    entry: Any,
    version: str,
) -> None:
    """Update string-style dependency ``dependency`` in ``deps``.

    Examples
    --------
    >>> import tomlkit
    >>> doc = tomlkit.parse('foo = "^0.1"')
    >>> _update_string_dependency(doc, 'foo', doc['foo'], '1.2.3')
    >>> tomlkit.dumps(doc).strip()
    'foo = "^1.2.3"'
    """
    prefix = _extract_version_prefix(entry)
    if isinstance(entry, tomlkit.items.String):
        new = tomlkit.string(prefix + version)
        new.trivia.indent = entry.trivia.indent
        new.trivia.comment_ws = entry.trivia.comment_ws
        new.trivia.comment = entry.trivia.comment
        new.trivia.trail = entry.trivia.trail
        deps[dependency] = new
    else:
        deps[dependency] = prefix + version


def _update_dependency_in_table(
    deps: MutableMapping[str, object],
    dependency: str,
    version: str,
) -> None:
    """Update ``dependency`` inside dependency table ``deps``.

    Examples
    --------
    >>> import tomlkit
    >>> doc = tomlkit.parse('[dependencies]\nfoo = "^0.1"')
    >>> _update_dependency_in_table(doc['dependencies'], 'foo', '1.2.3')
    >>> 'foo = "^1.2.3"' in tomlkit.dumps(doc)
    True
    """
    entry = deps[dependency]
    if isinstance(entry, Mapping):
        _update_dict_dependency(entry, version)
    else:
        _update_string_dependency(deps, dependency, entry, version)


def _update_dependency_version(
    doc: MutableMapping[str, object],
    dependency: str,
    version: str,
) -> None:
    """Update ``dependency`` across dependency tables in ``doc``.

    Maintains caret or tilde prefixes and preserves formatting.

    Examples
    --------
    >>> doc = tomlkit.parse('[dependencies]\nfoo = "^0.1"')
    >>> _update_dependency_version(doc, 'foo', '1.2.3')
    >>> tomlkit.dumps(doc).strip()
    '[dependencies]\nfoo = "^1.2.3"'

    >>> snippet = '[dependencies]\nfoo = { version = "~0.1", features = ["a"] }'
    >>> doc = tomlkit.parse(snippet)
    >>> _update_dependency_version(doc, 'foo', '1.2.3')
    >>> 'version = "~1.2.3"' in tomlkit.dumps(doc)
    True

    >>> doc = tomlkit.parse('[dev-dependencies]\nfoo = "^0.1"')
    >>> _update_dependency_version(doc, 'foo', '1.2.3')
    >>> 'foo = "^1.2.3"' in tomlkit.dumps(doc)
    True
    """
    for table in ("dependencies", "dev-dependencies", "build-dependencies"):
        deps = doc.get(table)
        if not deps or dependency not in deps:
            continue
        _update_dependency_in_table(deps, dependency, version)


def _set_version(
    toml_path: Path,
    version: str,
    dependency: str | None = None,
    doc: MutableMapping[str, object] | None = None,
) -> None:
    """Set package and optional dependency version in a ``Cargo.toml``.

    Parameters
    ----------
    toml_path
        Path to the ``Cargo.toml`` file.
    version
        Version string to apply.
    dependency
        Optional dependency to update alongside the package version.
    doc
        Pre-parsed document to update. If provided, the file is not re-read.
    """
    if doc is None:
        with toml_path.open("r", encoding="utf-8") as fh:
            doc = tomlkit.parse(fh.read())

    _update_package_version(doc, version)

    if dependency:
        _update_dependency_version(doc, dependency, version)

    text = tomlkit.dumps(doc)
    temp_dir = toml_path.parent
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", dir=temp_dir, delete=False
    ) as tf:
        tf.write(text)
        temp_name = tf.name
    os.replace(temp_name, toml_path)


def _validate_args_and_setup(argv: list[str]) -> tuple[str, Path] | None:
    """Validate CLI arguments and resolve the workspace root.

    Parameters
    ----------
    argv
        Raw command-line arguments.

    Returns
    -------
    tuple[str, Path] | None
        The requested version and repository root if arguments are valid;
        ``None`` otherwise.

    Examples
    --------
    >>> _validate_args_and_setup(["bump_version.py", "1.2.3"])  # doctest: +ELLIPSIS
    ('1.2.3', Path(...))
    """
    if len(argv) != 2:
        prog = Path(argv[0]).name
        print(f"Usage: {prog} <version>", file=sys.stderr)
        return None
    version = argv[1]
    root = Path(__file__).resolve().parent.parent
    return version, root


def _resolve_member_paths(root: Path, members: list[str]) -> list[Path]:
    """Expand workspace member patterns to concrete paths.

    Parameters
    ----------
    root
        Workspace root directory.
    members
        Glob patterns for workspace members.

    Returns
    -------
    list[Path]
        Paths matched by the supplied patterns. Warnings are emitted for
        patterns that match nothing.

    Examples
    --------
    >>> _resolve_member_paths(Path('.'), ['scripts'])  # doctest: +ELLIPSIS
    [Path('scripts')]
    """
    paths: list[Path] = []
    for pattern in members:
        matches = list(root.glob(pattern))
        if not matches:
            print(
                f"Warning: No members matched pattern '{pattern}'",
                file=sys.stderr,
            )
            continue
        paths.extend(matches)
    return paths


def _update_member_version(member_path: Path, version: str) -> bool:
    """Update a member Cargo.toml with the supplied version.

    Parameters
    ----------
    member_path
        Path to the member's ``Cargo.toml`` file.
    version
        Semantic version to apply.

    Returns
    -------
    bool
        ``True`` if an error occurred while updating; ``False`` otherwise.

    Examples
    --------
    >>> _update_member_version(Path('Cargo.toml'), '1.2.3')
    False
    """
    try:
        # Derive from the actual package name to avoid coupling to directory names
        with member_path.open("r", encoding="utf-8") as fh:
            doc = tomlkit.parse(fh.read())
        package_name = doc.get("package", {}).get("name")
        dep = "ortho_config_macros" if package_name == "ortho_config" else None
        _set_version(member_path, version, dep, doc)
    except (
        TOMLKitError,
        OSError,
        TypeError,
        ValueError,
    ) as exc:  # pragma: no cover - defensive
        print(
            f"Error: Failed to set version for {member_path}: {exc}",
            file=sys.stderr,
        )
        return True
    return False


def _process_single_member(member_root: Path, version: str) -> bool:
    """Process a single workspace member.

    Parameters
    ----------
    member_root
        Directory or file matched from the workspace ``members`` patterns.
    version
        Semantic version to apply.

    Returns
    -------
    bool
        ``True`` if updating the member failed; ``False`` otherwise.

    Examples
    --------
    >>> _process_single_member(Path('scripts'), '1.2.3')
    False
    """
    member_path = (
        member_root / "Cargo.toml" if member_root.is_dir() else member_root
    )
    if not member_path.exists():
        print(
            f"Warning: Skipping missing member Cargo.toml at {member_path}",
            file=sys.stderr,
        )
        return False
    return _update_member_version(member_path, version)


def _process_members(root: Path, members: list[str], version: str) -> bool:
    """Update all workspace members to the supplied version.

    Parameters
    ----------
    root
        Workspace root directory.
    members
        Glob patterns for workspace members.
    version
        Semantic version to apply.

    Returns
    -------
    bool
        ``True`` if any member failed to update; ``False`` otherwise.

    Examples
    --------
    >>> _process_members(Path('.'), ['scripts'], '1.2.3')
    False
    """
    had_error = False
    for member_root in _resolve_member_paths(root, members):
        if _process_single_member(member_root, version):
            had_error = True
    return had_error


def _replace_version_in_toml(snippet: str, version: str) -> str:
    """Update ``ortho_config`` version in TOML snippet, preserving formatting."""
    try:
        doc = tomlkit.parse(snippet)
    except TOMLKitError:
        return snippet
    for table in ("dependencies", "dev-dependencies", "build-dependencies"):
        deps = doc.get(table)
        if deps and "ortho_config" in deps:
            _update_dependency_in_table(deps, "ortho_config", version)
    return tomlkit.dumps(doc).rstrip()


def _update_markdown_versions(md_path: Path, version: str) -> None:
    """Update ``ortho_config`` versions in TOML fences within ``md_path``.

    Examples
    --------
    >>> import tempfile
    >>> sample = '```toml\n[dependencies]\northo_config = "0"\n```\n'
    >>> with tempfile.NamedTemporaryFile('w+', suffix='.md') as fh:
    ...     _ = fh.write(sample)
    ...     fh.flush()
    ...     _update_markdown_versions(Path(fh.name), '1')
    ...     fh.seek(0)
    ...     'ortho_config = "1"' in fh.read()
    True
    """
    if not md_path.exists():
        return
    text = md_path.read_text(encoding="utf-8")
    updated = replace_fences(text, "toml", lambda body: _replace_version_in_toml(body, version))
    md_path.write_text(updated, encoding="utf-8")


def main(argv: list[str]) -> int:
    """
    Update the workspace and member crate versions to the supplied value.

    Parameters
    ----------
    argv
        Command-line arguments where `argv[1]` is the target semantic version
        (for example, "1.2.3").

    Returns
    -------
    int
        Zero on success; non-zero if any member update fails or arguments are
        invalid.

    Examples
    --------
    >>> import sys
    >>> sys.exit(main(["bump_version.py", "1.2.3"]))
    """
    result = _validate_args_and_setup(argv)
    if result is None:
        return 1
    version, root = result
    workspace = root / "Cargo.toml"
    try:
        with workspace.open("r", encoding="utf-8") as fh:
            data = tomlkit.parse(fh.read())
    except (TOMLKitError, OSError, TypeError, ValueError) as exc:  # pragma: no cover - defensive
        print(f"Error: Failed to parse {workspace}: {exc}", file=sys.stderr)
        return 1
    members = data.get("workspace", {}).get("members", [])
    _set_version(workspace, version, doc=data)
    had_error = _process_members(root, members, version)
    _update_markdown_versions(root / "README.md", version)
    _update_markdown_versions(root / "docs" / "users-guide.md", version)
    return 0 if not had_error else 1

if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main(sys.argv))
