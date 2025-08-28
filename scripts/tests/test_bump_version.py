from pathlib import Path

import pytest
import tomlkit

from scripts.bump_version import (
    _update_dependency_version,
    _update_markdown_versions,
    replace_fences,
)


@pytest.mark.parametrize(
    "section",
    ["dependencies", "dev-dependencies", "build-dependencies"],
)
@pytest.mark.parametrize(
    "body, expected, extra",
    [
        ("foo = \"^0.1\"", "foo = \"^1.2.3\"", None),
        (
            'foo = { version = "~0.1", features = ["a"] }',
            'version = "~1.2.3"',
            'features = ["a"]',
        ),
    ],
)
def test_updates_dependency_version(
    section: str, body: str, expected: str, extra: str | None
) -> None:
    doc = tomlkit.parse(f"[{section}]\n{body}")
    _update_dependency_version(doc, "foo", "1.2.3")
    dumped = tomlkit.dumps(doc)
    assert expected in dumped
    if extra:
        assert extra in dumped


def test_preserves_trailing_comment_on_string_dependency() -> None:
    doc = tomlkit.parse('[dependencies]\nfoo = "^0.1"  # pinned for CI\n')
    _update_dependency_version(doc, 'foo', '1.2.3')
    dumped = tomlkit.dumps(doc)
    assert '# pinned for CI' in dumped, "must preserve trailing comment on value node"


def test_preserves_quote_style_on_string_dependency() -> None:
    doc = tomlkit.parse("[dependencies]\nfoo = '0.1'  # single quoted\n")
    _update_dependency_version(doc, "foo", "1.2.3")
    dumped = tomlkit.dumps(doc)
    assert "foo = '1.2.3'" in dumped, (
        f"expected single quotes preserved, got:\n{dumped}"
    )


def test_missing_dependency_no_change() -> None:
    snippet = '[dependencies]\nbar = "0.1"'
    doc = tomlkit.parse(snippet)
    _update_dependency_version(doc, 'foo', '1.2.3')
    assert tomlkit.dumps(doc).strip() == snippet, "should be a no-op when dependency is absent"


def test_update_markdown_versions_updates_toml_fences(tmp_path: Path) -> None:
    md_text = """pre
```toml
[dependencies]
ortho_config = "0"
```
post
"""
    for rel in ("README.md", "docs/users-guide.md"):
        md_path = tmp_path / rel
        md_path.parent.mkdir(parents=True, exist_ok=True)
        md_path.write_text(md_text)
        _update_markdown_versions(md_path, "1")
        updated = md_path.read_text()
        assert 'ortho_config = "1"' in updated, "must update TOML fences"


def test_update_markdown_versions_ignores_non_toml_fences(tmp_path: Path) -> None:
    md_text = "pre\n```bash\necho hi\n```\npost\n"
    for rel in ("README.md", "docs/users-guide.md"):
        md_path = tmp_path / rel
        md_path.parent.mkdir(parents=True, exist_ok=True)
        md_path.write_text(md_text)
        _update_markdown_versions(md_path, "1")
        assert md_path.read_text() == md_text, "must leave non-TOML fences unchanged"


def test_replace_fences_preserves_indentation() -> None:
    md_text = (
        "1. item\n\n"
        "    ```toml\n"
        "    [dependencies]\n"
        '    foo = "0"\n'
        "    ```\n"
    )
    replaced = replace_fences(md_text, "toml", lambda body: body.replace("0", "1"))
    expected = (
        "1. item\n\n"
        "    ```toml\n"
        "    [dependencies]\n"
        '    foo = "1"\n'
        "    ```\n"
    )
    assert replaced == expected
