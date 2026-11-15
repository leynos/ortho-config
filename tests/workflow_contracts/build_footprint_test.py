"""Contract test for the development build footprint rule.

The Concordat ``rust-dev-build-footprint`` rule needs the default development
profile to disable incremental compilation and debug information. The profile
lives in the workspace ``Cargo.toml``, so an edit there must not quietly
re-enable either.

Run via ``make test-workflow-contracts``.
"""

import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def test_development_profile_keeps_the_build_footprint_settings() -> None:
    """The dev profile sets ``incremental = false`` and ``debug = 0``."""
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text("utf-8"))
    dev = manifest.get("profile", {}).get("dev", {})
    assert dev.get("incremental") is False, "profile.dev must set incremental = false"
    debug = dev.get("debug")
    # Python equality would also accept ``0.0``, which Cargo rejects, so test the
    # type: the exact integer ``0`` or the boolean ``false``.
    is_zero_integer = isinstance(debug, int) and not isinstance(debug, bool) and debug == 0
    assert debug is False or is_zero_integer, "profile.dev must set debug = 0 or false"
