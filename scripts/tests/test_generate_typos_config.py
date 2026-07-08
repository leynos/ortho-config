"""Tests for the en-GB-oxendict ``typos.toml`` generator."""

import pytest

from scripts import generate_typos_config
from scripts.generate_typos_config import render_config


def test_render_config_emits_paired_oxford_entries() -> None:
    """Each curated stem yields an -ise correction and an -ize identity key."""
    config = render_config()
    assert 'organise = "organize"' in config
    assert 'organize = "organize"' in config


def test_render_config_rejects_duplicate_keys(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Colliding stems fail loudly instead of emitting a duplicate TOML key."""
    monkeypatch.setattr(generate_typos_config, "STEMS", ("token", "token"))
    with pytest.raises(ValueError, match="duplicate typos key"):
        render_config()
