"""Security contracts for authority and local spelling policy."""

from __future__ import annotations

import email.message
import re
import types
import urllib.error
from pathlib import Path

import pytest

from typos_rollout_test_support import dictionary_text

REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
type RolloutModules = tuple[types.ModuleType, types.ModuleType, types.ModuleType]


@pytest.mark.parametrize("schema", ["true", "1.0"])
def test_authority_requires_integer_schema(
    rollout_modules: RolloutModules,
    tmp_path: Path,
    schema: str,
) -> None:
    """Boolean and floating-point schemas cannot validate as version one."""
    _, rollout, _ = rollout_modules
    authority = tmp_path / "base.toml"
    authority.write_text(
        dictionary_text().replace("schema = 1", f"schema = {schema}"),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="unsupported dictionary schema"):
        rollout.load_dictionary(authority)


@pytest.mark.parametrize(
    ("fragment", "message"),
    [
        ("[phrases.corrections]\n\n", "missing required table 'phrases'"),
        ("exclude = []\n", "missing required field files.exclude"),
    ],
)
def test_authority_requires_complete_policy(
    rollout_modules: RolloutModules,
    tmp_path: Path,
    fragment: str,
    message: str,
) -> None:
    """Every authority table and expected field is mandatory."""
    _, rollout, _ = rollout_modules
    authority = tmp_path / "base.toml"
    authority.write_text(dictionary_text().replace(fragment, ""), encoding="utf-8")

    with pytest.raises(ValueError, match=message):
        rollout.load_dictionary(authority)


def test_explicit_sparse_local_overlay_remains_valid(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """Only explicit local loads may omit unrelated policy tables."""
    _, rollout, _ = rollout_modules
    overlay = tmp_path / "typos.local.toml"
    overlay.write_text(
        'schema = 1\n\n[words]\naccepted = ["LocalTerm"]\n',
        encoding="utf-8",
    )

    parsed = rollout.load_dictionary(overlay, local_overlay=True)

    assert parsed.accepted == ("LocalTerm",), "sparse local vocabulary was lost"
    assert parsed.stems == (), "sparse local load invented authority policy"


@pytest.mark.parametrize(
    ("ignore_patterns", "excluded_files"),
    [
        pytest.param((".*",), (), id="empty-matching-ignore"),
        pytest.param((".+",), (), id="generic-prose-ignore"),
        pytest.param(("[A-Za-z]+",), (), id="prose-word-substring-ignore"),
        pytest.param(("ordinary",), (), id="literal-prose-substring-ignore"),
        pytest.param((), ("*",), id="all-files-exclusion"),
        pytest.param((), ("**/*",), id="universal-exclusion"),
        pytest.param((), ("*.md",), id="top-level-markdown-exclusion"),
        pytest.param((), ("**.md",), id="path-match-universal-exclusion"),
        pytest.param((), ("**/*.md",), id="recursive-markdown-exclusion"),
        pytest.param((), (" ./**/*.MD ",), id="normalized-markdown-exclusion"),
    ],
)
def test_merge_rejects_broad_local_exceptions(
    rollout_modules: RolloutModules,
    ignore_patterns: tuple[str, ...],
    excluded_files: tuple[str, ...],
) -> None:
    """Local exceptions cannot disable estate policy broadly."""
    _, rollout, _ = rollout_modules
    local = rollout.Dictionary(
        ignore_patterns=ignore_patterns,
        excluded_files=excluded_files,
    )

    with pytest.raises(ValueError, match="too broad"):
        rollout.merge_dictionaries(rollout.Dictionary(), local)


@pytest.mark.parametrize(
    ("pattern", "message"),
    [
        pytest.param("[", "invalid", id="malformed"),
        pytest.param("(a+)+$", "unsafe repetition", id="nested-repetition"),
        pytest.param("(a|aa)+$", "unsafe repetition", id="repeated-alternation"),
        pytest.param("a*a*$", "unsafe repetition", id="adjacent-repetitions"),
    ],
)
def test_authority_rejects_unsafe_ignore_patterns(
    rollout_modules: RolloutModules,
    tmp_path: Path,
    pattern: str,
    message: str,
) -> None:
    """Every authority regex must compile with bounded matching complexity."""
    _, rollout, _ = rollout_modules
    authority = tmp_path / "base.toml"
    authority.write_text(
        dictionary_text().replace("ignore = []", f"ignore = [{pattern!r}]"),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match=message) as raised:
        rollout.load_dictionary(authority)

    if pattern == "[":
        assert isinstance(raised.value.__cause__, re.error), (
            "malformed regex did not preserve its parser failure as the cause"
        )


def test_local_overlay_accepts_separated_safe_repetitions(
    rollout_modules: RolloutModules,
) -> None:
    """Separated whitespace repetitions remain valid narrow exceptions."""
    _, rollout, _ = rollout_modules

    local = rollout.load_dictionary(
        REPOSITORY_ROOT / "typos.local.toml",
        local_overlay=True,
    )

    assert r"GitHub\s+Flavored\s+Markdown" in local.ignore_patterns, (
        "the existing GitHub Markdown exception was rejected"
    )


def test_merge_accepts_repository_local_exceptions(
    rollout_modules: RolloutModules,
) -> None:
    """Existing repository exceptions remain valid narrow additions."""
    _, rollout, _ = rollout_modules
    local = rollout.load_dictionary(
        REPOSITORY_ROOT / "typos.local.toml",
        local_overlay=True,
    )

    merged = rollout.merge_dictionaries(rollout.Dictionary(), local)

    assert merged.ignore_patterns == local.ignore_patterns, (
        "repository ignore patterns were dropped during merge"
    )
    assert merged.excluded_files == local.excluded_files, (
        "repository file exclusions were dropped during merge"
    )


def _not_modified_error() -> urllib.error.HTTPError:
    """Build the production representation of an HTTP 304 response."""
    return urllib.error.HTTPError(
        "https://example.test/base",
        304,
        "not modified",
        email.message.Message(),
        None,
    )


def test_http_304_requires_valid_cache(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """The HTTPError 304 path cannot accept a missing cache."""
    _, rollout, _ = rollout_modules
    not_modified = _not_modified_error()

    with pytest.raises(urllib.error.HTTPError) as raised:
        rollout.refresh_base(
            "https://example.test/base",
            tmp_path / "cache.toml",
            rollout.RefreshOptions(
                metadata=tmp_path / "cache.json",
                opener=lambda *_args, **_kwargs: (_ for _ in ()).throw(not_modified),
            ),
        )

    assert raised.value is not_modified, "HTTP 304 accepted a missing cache"


def test_http_304_preserves_valid_cache(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """The HTTPError 304 path reports a validated cache as current."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    cache.write_text(dictionary_text(), encoding="utf-8")
    metadata = tmp_path / "cache.json"
    metadata.write_text(
        '{"source": "https://example.test/base"}\n',
        encoding="utf-8",
    )
    not_modified = _not_modified_error()

    result = rollout.refresh_base(
        "https://example.test/base",
        cache,
        rollout.RefreshOptions(
            metadata=metadata,
            opener=lambda *_args, **_kwargs: (_ for _ in ()).throw(not_modified),
        ),
    )

    assert result.status == "current", "HTTP 304 did not preserve a valid cache"
    assert result.cache == cache, "HTTP 304 returned a different cache path"


def test_http_304_rejects_cache_from_different_source(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """HTTP 304 cannot validate a cache written for another authority."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    cache.write_text(dictionary_text(), encoding="utf-8")
    metadata = tmp_path / "cache.json"
    metadata.write_text(
        '{"source": "https://other.example.test/base"}\n',
        encoding="utf-8",
    )
    not_modified = _not_modified_error()

    with pytest.raises(urllib.error.HTTPError) as raised:
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=metadata,
                opener=lambda *_args, **_kwargs: (_ for _ in ()).throw(not_modified),
            ),
        )

    assert raised.value is not_modified, "HTTP 304 accepted another source's cache"


def test_http_304_rejects_valid_cache_without_source_metadata(
    rollout_modules: RolloutModules,
    tmp_path: Path,
) -> None:
    """HTTP 304 cannot validate a cache whose authority is unknown."""
    _, rollout, _ = rollout_modules
    cache = tmp_path / "cache.toml"
    cache.write_text(dictionary_text(), encoding="utf-8")
    not_modified = _not_modified_error()

    with pytest.raises(urllib.error.HTTPError) as raised:
        rollout.refresh_base(
            "https://example.test/base",
            cache,
            rollout.RefreshOptions(
                metadata=tmp_path / "cache.json",
                opener=lambda *_args, **_kwargs: (_ for _ in ()).throw(not_modified),
            ),
        )

    assert raised.value is not_modified, "HTTP 304 accepted an unscoped cache"
