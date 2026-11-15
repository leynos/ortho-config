"""Contract for the Windows trybuild exclusivity override.

On Windows, the first coverage pass runs four tests that each drive a
cold child ``cargo`` build through trybuild. Run side by side, they
exhausted the 600s per-test allowance: every Windows failure in runs
36127709137, 36127710357, 36127710423, 36127710779 and 36127709460 was
``cargo-orthohelp::compile_time``'s ``must_use_compile_tests`` at 600s,
overlapping ``ortho_config::compile_fail`` and ``crate_path_trybuild``.

``.config/nextest.toml`` therefore reserves every nextest slot for each
of those binaries on Windows, and leaves the ceiling and Linux alone.
This module holds that shape: the exact binary set, the platform, the
reservation, and the unchanged allowance. See "Nextest test-group
serialization" in ``docs/developers-guide.md``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import tomllib
import typing as typ

import pytest

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
NEXTEST_CONFIG: typ.Final[pathlib.Path] = REPOSITORY_ROOT / ".config" / "nextest.toml"

#: Every binary whose test drives a cold trybuild build, exactly.
TRYBUILD_BINARIES: typ.Final[frozenset[str]] = frozenset({
    "binary_id(ortho_config::crate_path_trybuild)",
    "binary_id(ortho_config::declarative_merge_trybuild)",
    "binary_id(ortho_config::compile_fail)",
    "binary_id(cargo-orthohelp::compile_time)",
})

WINDOWS: typ.Final[str] = "cfg(windows)"
EXCLUSIVE: typ.Final[str] = "num-test-threads"

#: The trybuild allowance, unchanged by the exclusivity: 120s x 5 = 600s.
TRYBUILD_TIMEOUT: typ.Final[dict[str, object]] = {"period": "120s", "terminate-after": 5}


@pytest.fixture(scope="module")
def overrides() -> list[dict[str, object]]:
    """Return the default profile's ``[[overrides]]`` entries, parsed once.

    Returns
    -------
    list of dict
        The entries in declaration order.
    """
    document = tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))
    entries = document.get("profile", {}).get("default", {}).get("overrides", [])
    return [entry for entry in entries if isinstance(entry, dict)]


def _terms(filterset: object) -> frozenset[str]:
    """Return a filterset's ``|`` terms, whitespace-normalized."""
    return frozenset("".join(term.split()) for term in str(filterset).split("|"))


def _windows_exclusive(overrides: list[dict[str, object]]) -> dict[str, object]:
    """Return the single Windows override that reserves every slot."""
    found = [
        entry
        for entry in overrides
        if entry.get("platform") == WINDOWS and "threads-required" in entry
    ]
    assert len(found) == 1, (
        f"expected one Windows override reserving nextest slots, found {len(found)}"
    )
    return found[0]


def test_every_cold_trybuild_binary_runs_alone_on_windows(
    overrides: list[dict[str, object]],
) -> None:
    """The Windows override names exactly the four trybuild binaries.

    Missing ``compile_time`` is the defect this guards: the first form
    of the override named only the two ``*_trybuild`` binaries, and the
    test that actually timed out stayed free to overlap the others.
    """
    entry = _windows_exclusive(overrides)
    assert _terms(entry.get("filter")) == TRYBUILD_BINARIES, (
        f"the Windows exclusivity must name exactly {sorted(TRYBUILD_BINARIES)}; "
        f"it names {sorted(_terms(entry.get('filter')))}"
    )
    assert entry.get("threads-required") == EXCLUSIVE, (
        f"each trybuild binary must reserve every slot ({EXCLUSIVE!r}); the "
        f"override asks for {entry.get('threads-required')!r}"
    )


def test_the_exclusivity_keeps_the_600s_ceiling(
    overrides: list[dict[str, object]],
) -> None:
    """Running alone is the remedy, not a longer allowance."""
    entry = _windows_exclusive(overrides)
    assert entry.get("slow-timeout") == TRYBUILD_TIMEOUT, (
        f"the Windows trybuild allowance must stay {TRYBUILD_TIMEOUT}; it is "
        f"{entry.get('slow-timeout')!r}"
    )


def test_no_other_override_reserves_slots(
    overrides: list[dict[str, object]],
) -> None:
    """Linux and every other platform keep their execution policy."""
    others = [
        entry
        for entry in overrides
        if "threads-required" in entry and entry.get("platform") != WINDOWS
    ]
    assert not others, f"only the Windows override may reserve slots: {others}"
