import tomlkit
from scripts.bump_version import _update_dependency_version


def test_updates_string_dependency_preserving_prefix():
    doc = tomlkit.parse('[dependencies]\nfoo = "^0.1"')
    _update_dependency_version(doc, 'foo', '1.2.3')
    assert 'foo = "^1.2.3"' in tomlkit.dumps(doc)


def test_updates_dict_dependency_preserving_prefix():
    snippet = '[dependencies]\nfoo = { version = "~0.1", features = ["a"] }'
    doc = tomlkit.parse(snippet)
    _update_dependency_version(doc, 'foo', '1.2.3')
    dumped = tomlkit.dumps(doc)
    assert 'version = "~1.2.3"' in dumped
    assert 'features = ["a"]' in dumped


def test_updates_dev_dependency_table():
    doc = tomlkit.parse('[dev-dependencies]\nfoo = "0.1"')
    _update_dependency_version(doc, 'foo', '1.2.3')
    assert 'foo = "1.2.3"' in tomlkit.dumps(doc)


def test_missing_dependency_no_change():
    snippet = '[dependencies]\nbar = "0.1"'
    doc = tomlkit.parse(snippet)
    _update_dependency_version(doc, 'foo', '1.2.3')
    assert tomlkit.dumps(doc).strip() == snippet
