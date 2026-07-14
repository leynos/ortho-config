"""Compatibility tests for the established spelling-config generator API."""

from pathlib import Path

import pytest

import generate_typos_config as generator
import typos_rollout as rollout


def _dictionary(
    _repository: Path = generator.REPOSITORY_ROOT,
) -> rollout.Dictionary:
    """Return a deterministic dictionary for compatibility tests."""
    return rollout.Dictionary(stems=("organ",))


def test_render_config_keeps_the_no_argument_api(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The established no-argument renderer still emits Oxford entries."""
    monkeypatch.setattr(generator, "dictionary_from_cache", _dictionary)

    config = generator.render_config()

    assert '"organise" = "organize"' in config, "Oxford correction was omitted"
    assert '"organize" = "organize"' in config, "Oxford spelling was omitted"


def test_main_keeps_the_positional_output_api(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """A positional destination still receives the generated configuration."""
    output = tmp_path / "custom-typos.toml"
    refresh = rollout.RefreshResult("current", tmp_path / "base.toml")

    monkeypatch.setattr(generator, "dictionary_from_cache", _dictionary)
    monkeypatch.setattr(rollout, "refresh_base", lambda *_args, **_kwargs: refresh)

    result = generator.main(output)

    assert result == refresh, "generator did not return the refresh result"
    assert '"organise" = "organize"' in output.read_text(encoding="utf-8"), (
        "positional output omitted the Oxford correction"
    )
