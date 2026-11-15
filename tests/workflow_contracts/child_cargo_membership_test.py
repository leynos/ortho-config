"""Source-backed membership tests for the child-cargo tier.

These tests keep the Rust source reading independent from nextest's filter
text. They cover marker liveness and a controlled member absent from the
configuration, so the contract cannot pass by deriving its class from that
configuration.
"""

from __future__ import annotations

import pathlib

from child_cargo_tier import (
    CHILD_CARGO_TIER_CRATES,
    child_cargo_binaries,
    marker_binaries,
    non_child_cargo_binaries,
)
from nextest_precedence import effective_allowances, selected_by_universe

REPOSITORY_ROOT = pathlib.Path(__file__).resolve().parents[2]
NEXTEST_CONFIG = REPOSITORY_ROOT / ".config" / "nextest.toml"


def test_every_marker_still_selects_a_binary() -> None:
    """Each marker earns its place in the reading.

    A marker whose subject was renamed matches nothing and silently shrinks the
    class, which the coverage assertion above cannot report: it compares the
    class with the filter, and both agree once the member is gone. The
    ``cargo-runner`` marker did exactly that when the shared helper was renamed
    from ``prepare_cargo_command``, and the union still held ten members, so
    nothing else noticed.

    Liveness is asserted **workspace-wide**, not per crate, because the markers
    are not per-crate facts: ``cargo_orthohelp_exe`` is a
    ``cargo-orthohelp``-only export, so requiring it in ``ortho_config`` would
    demand a ``binary-loader`` hit that no honest source can supply. What each
    label must do is select at least one binary somewhere in the audited
    crates, since that is what keeps its members in the class.
    """
    per_marker = marker_binaries(REPOSITORY_ROOT)
    assert per_marker, "the reader must report a group per marker"
    empty = sorted(label for label, hits in per_marker.items() if not hits)
    assert not empty, (
        f"{empty} select no binary in any audited crate; either the helper "
        f"each names was renamed, or the repository stopped spawning cargo "
        f"that way. A marker that matches nothing removes its members from the "
        f"class, and the coverage assertion cannot see that because it compares "
        f"the class with a filter that agrees with it"
    )
    union: set[str] = set()
    for hits in per_marker.values():
        union |= hits
    assert union == set(child_cargo_binaries(REPOSITORY_ROOT)), (
        "the per-marker sets and the class read from the same files disagree; "
        "one of the two readings is not reporting what the module graph says"
    )


def _fixture_workspace(tmp_path: pathlib.Path, binary: str) -> pathlib.Path:
    """Write a minimal workspace whose one test binary drives a child cargo.

    Two crates are written because the reader walks a fixed list of them; the
    first carries the binary and the second is empty. The binary reaches its
    spawn through a helper in a submodule, so the fixture exercises the module
    graph the markers are read over rather than only a flat file.

    The helper sits in ``support/`` rather than beside the binary, which is
    both the layout the real repository uses for ``tests/fixtures`` and the one
    cargo agrees with: a ``.rs`` file directly in ``tests/`` is compiled as its
    own test binary, which is why shared helpers are put in a subdirectory. A
    fixture with ``helper.rs`` beside the binary would be read as two binaries,
    correctly.

    Parameters
    ----------
    tmp_path : pathlib.Path
        The test's temporary directory.
    binary : str
        The stem to give the test binary.

    Returns
    -------
    pathlib.Path
        The workspace root, suitable as the reader's ``base``.
    """
    for crate in CHILD_CARGO_TIER_CRATES:
        (tmp_path / crate / "tests" / "support").mkdir(parents=True)
        (tmp_path / crate / "Cargo.toml").write_text("[package]\n", encoding="utf-8")
    roots = tmp_path / CHILD_CARGO_TIER_CRATES[0] / "tests"
    (roots / f"{binary}.rs").write_text("mod support;\n", encoding="utf-8")
    (roots / "support" / "mod.rs").write_text(
        'fn run() {\n    let _ = Command::new("cargo");\n}\n', encoding="utf-8"
    )
    return tmp_path


def test_the_class_is_read_from_the_sources_not_from_the_filter(
    tmp_path: pathlib.Path,
) -> None:
    """The class follows the Rust, not the configuration naming it.

    The member list is a fact about the sources, and this asserts that by
    changing the sources while holding the configuration fixed. A reader that
    derived the class from the override's filter -- the reading that would make
    every other assertion in this module vacuous, since it would agree with the
    filter by construction -- reports the certified class for the second
    workspace below and fails here.

    The first part states the fact the other tests rest on for the real
    repository; the second is the controlled input that separates the two
    readings.
    """
    class_members = child_cargo_binaries(REPOSITORY_ROOT)
    assert class_members, "the child-cargo class must not be empty"
    non_members = non_child_cargo_binaries(REPOSITORY_ROOT)
    assert class_members & non_members == set(), (
        "a binary cannot both drive a child cargo and not drive one; the two "
        "readings disagree about which binaries exist"
    )
    # The fixture's source names a binary the configuration has never heard of.
    # The configuration is not consulted here at all, so a class that follows
    # the filter rather than the sources cannot produce this name.
    fixture = _fixture_workspace(tmp_path, "a_binary_no_override_names")
    assert child_cargo_binaries(fixture) == {"a_binary_no_override_names"}, (
        "the reader did not follow the fixture's sources; the class must come "
        "from what the Rust compiles, and the certified configuration names no "
        "such binary"
    )
    assert child_cargo_binaries(fixture) != class_members, (
        "the fixture reproduces the certified class, so it cannot show the "
        "reading follows the sources rather than the configuration"
    )


def test_a_source_backed_member_with_no_override_is_reported(
    tmp_path: pathlib.Path,
) -> None:
    """A member added to the sources but not the configuration fails the tier.

    The second controlled input: the configuration is held at its certified
    text and the class is changed, which is the omission issue #611 recorded
    for ``cli_dispatch`` and the direction a hand-maintained entry list drifts
    in. The assertion is the same one the coverage test makes, so this shows
    that assertion failing for the reason it exists rather than only passing.
    """
    certified = NEXTEST_CONFIG.read_text(encoding="utf-8")
    fixture = _fixture_workspace(tmp_path, "a_binary_no_override_names")
    universe = sorted(child_cargo_binaries(fixture) | {"cli_dispatch"})
    tiers = effective_allowances(
        certified,
        sorted(child_cargo_binaries(fixture)),
        reader=selected_by_universe(universe),
    )
    on_base = sorted(name for name, tier in tiers.items() if not tier.is_above_base)
    assert on_base == ["a_binary_no_override_names"], (
        f"a class member the configuration does not name resolved to {on_base}; "
        f"the fixture's binary has no override, so it must be the only member "
        f"on the profile's own allowance, exactly as cli_dispatch was"
    )
    assert not any(tier.is_unresolved for tier in tiers.values()), (
        "no entry in the certified configuration is unreadable, so a member "
        "must not be reported as unresolved"
    )
