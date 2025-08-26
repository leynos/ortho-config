import tomlkit
from scripts.bump_version import _update_dependency_version


def test_updates_string_dependency_preserving_prefix() -> None:
    doc = tomlkit.parse('[dependencies]\nfoo = "^0.1"')
    _update_dependency_version(doc, 'foo', '1.2.3')
    assert 'foo = "^1.2.3"' in tomlkit.dumps(doc), "should update caret string dependency"


def test_updates_dict_dependency_preserving_prefix() -> None:
    snippet = '[dependencies]\nfoo = { version = "~0.1", features = ["a"] }'
    doc = tomlkit.parse(snippet)
    _update_dependency_version(doc, 'foo', '1.2.3')
    dumped = tomlkit.dumps(doc)
    assert 'version = "~1.2.3"' in dumped, "should update dict dependency version"
    assert 'features = ["a"]' in dumped, "should preserve other fields"


def test_updates_dev_dependency_table() -> None:
    doc = tomlkit.parse('[dev-dependencies]\nfoo = "0.1"')
    _update_dependency_version(doc, 'foo', '1.2.3')
    assert 'foo = "1.2.3"' in tomlkit.dumps(doc), "should update dev-dependencies table"


def test_updates_build_dependency_table() -> None:
    doc = tomlkit.parse('[build-dependencies]\nfoo = "^0.1"')
    _update_dependency_version(doc, 'foo', '1.2.3')
    assert 'foo = "^1.2.3"' in tomlkit.dumps(doc), "should update build-dependencies table"


def test_preserves_trailing_comment_on_string_dependency() -> None:
    doc = tomlkit.parse('[dependencies]\nfoo = "^0.1"  # pinned for CI\n')
    _update_dependency_version(doc, 'foo', '1.2.3')
    dumped = tomlkit.dumps(doc)
    assert '# pinned for CI' in dumped, "must preserve trailing comment on value node"


def test_missing_dependency_no_change() -> None:
    snippet = '[dependencies]\nbar = "0.1"'
    doc = tomlkit.parse(snippet)
    _update_dependency_version(doc, 'foo', '1.2.3')
    assert tomlkit.dumps(doc).strip() == snippet, "should be a no-op when dependency is absent"
