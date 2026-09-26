"""Contract for the Windows trybuild serialization group.

On Windows, the first coverage pass runs four tests that each drive a
cold child ``cargo`` build through trybuild. Run side by side, they
exhausted the 600s per-test allowance: every Windows failure in runs
36127709137, 36127710357, 36127710423, 36127710779 and 36127709460 was
``cargo-orthohelp::compile_time``'s ``must_use_compile_tests`` at 600s,
overlapping ``ortho_config::compile_fail`` and ``crate_path_trybuild``.

``.config/nextest.toml`` therefore puts exactly those binaries in one
single-threaded test group on Windows, so their child builds never
overlap one another while the rest of the suite keeps running, and it
leaves the ceiling and Linux alone. This module holds that shape. See
"Nextest test-group serialization" in ``docs/developers-guide.md``.

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
GROUP: typ.Final[str] = "windows_trybuild"

#: The trybuild allowance, unchanged by the grouping: 120s x 5 = 600s.
TRYBUILD_TIMEOUT: typ.Final[dict[str, object]] = {"period": "120s", "terminate-after": 5}


@pytest.fixture(scope="module")
def config() -> dict[str, object]:
    """Return ``.config/nextest.toml``, parsed once.

    Returns
    -------
    dict
        The parsed document.
    """
    return tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))


def _overrides(config: dict[str, object]) -> list[dict[str, object]]:
    """Return the default profile's ``[[overrides]]`` entries."""
    profile = config.get("profile", {})
    default = profile.get("default", {}) if isinstance(profile, dict) else {}
    entries = default.get("overrides", []) if isinstance(default, dict) else []
    return [entry for entry in entries if isinstance(entry, dict)]


def _terms(filterset: object) -> frozenset[str]:
    """Return a filterset's ``|`` terms, whitespace-normalized."""
    return frozenset("".join(term.split()) for term in str(filterset).split("|"))


def _group_members(config: dict[str, object]) -> list[dict[str, object]]:
    """Return every override assigning a test to the Windows trybuild group."""
    return [entry for entry in _overrides(config) if entry.get("test-group") == GROUP]


def test_the_windows_trybuild_group_admits_one_test_at_a_time(
    config: dict[str, object],
) -> None:
    """The group exists and runs its members one at a time."""
    groups = config.get("test-groups", {})
    group = groups.get(GROUP) if isinstance(groups, dict) else None
    assert isinstance(group, dict), f"nextest.toml must declare the {GROUP} test group"
    assert group.get("max-threads") == 1, (
        f"{GROUP} must run one test at a time; it allows {group.get('max-threads')!r}"
    )


def test_every_cold_trybuild_binary_joins_the_group_on_windows(
    config: dict[str, object],
) -> None:
    """One Windows override puts exactly the four trybuild binaries in the group.

    Missing ``compile_time`` is the defect this guards: the first form
    of the fix named only the two ``*_trybuild`` binaries, and the test
    that actually timed out stayed free to overlap the others.
    """
    members = _group_members(config)
    assert len(members) == 1, f"expected one override joining {GROUP}, found {len(members)}"
    (entry,) = members
    assert entry.get("platform") == WINDOWS, (
        f"{GROUP} membership must be scoped to {WINDOWS}; it is {entry.get('platform')!r}"
    )
    assert _terms(entry.get("filter")) == TRYBUILD_BINARIES, (
        f"{GROUP} must hold exactly {sorted(TRYBUILD_BINARIES)}; it holds "
        f"{sorted(_terms(entry.get('filter')))}"
    )


def test_the_grouping_keeps_the_600s_ceiling(config: dict[str, object]) -> None:
    """Running one at a time is the remedy, not a longer allowance."""
    (entry,) = _group_members(config)
    assert entry.get("slow-timeout") == TRYBUILD_TIMEOUT, (
        f"the Windows trybuild allowance must stay {TRYBUILD_TIMEOUT}; it is "
        f"{entry.get('slow-timeout')!r}"
    )


def test_no_override_reserves_every_slot(config: dict[str, object]) -> None:
    """The grouping replaced whole-run exclusivity, which cost 20 minutes."""
    reserving = [entry for entry in _overrides(config) if "threads-required" in entry]
    assert not reserving, f"no override may reserve nextest slots: {reserving}"
