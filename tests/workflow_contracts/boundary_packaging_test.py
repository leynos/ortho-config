"""Contract for the serialized ``boundary`` packaging group.

The boundary contracts materialize their packaged crates once per
*process*, and nextest runs each test in its own process. Several of them
therefore run ``cargo package`` and unpack the same target directory side
by side, and Cargo's own lock does not hold readers out while a package
root is removed and unpacked. One process can replace a root another is
reading, which shows up as an intermittent failure in a suite whose whole
purpose is to be the evidence for a published graph.

So ``.config/nextest.toml`` puts ``binary(boundary)`` in a single-threaded
group of its own, and the fixture's packaging deadline is sized to fit
inside the allowance that group runs under. Both halves are load-bearing
and neither is visible from the other file:

- a group that names a binary which does not exist serializes nothing,
  and reads exactly like one that works;
- a deadline longer than the per-test allowance can never fire, so the
  error naming the invocation that hung is never printed.

The deadline and its invocation count are read from the sources rather
than restated here, so the arithmetic and the code that enforces it
cannot drift apart. See "Nextest test-group serialization" in
``docs/developers-guide.md``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import tomllib
import typing as typ

from nextest_allowances import budget_of
from timeout_budgets import NEXTEST_CONFIG

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]

#: The binary whose tests share one packaging target directory.
BINARY: typ.Final[str] = "boundary"

#: The test group and the crate it belongs to.
GROUP: typ.Final[str] = "boundary_packaging"
CRATE: typ.Final[str] = "ortho_l10n"

#: The manifest declaring the binary, and the fixture declaring its deadline.
MANIFEST: typ.Final[pathlib.Path] = REPOSITORY_ROOT / CRATE / "Cargo.toml"
PACKAGE_SET: typ.Final[pathlib.Path] = (
    REPOSITORY_ROOT / CRATE / "tests" / "boundary" / "package_set" / "mod.rs"
)
CARGO_ENVIRONMENT: typ.Final[pathlib.Path] = (
    REPOSITORY_ROOT / CRATE / "tests" / "boundary" / "fixture_cargo" / "environment.rs"
)


def _overrides() -> list[dict[str, object]]:
    """Return the default profile's ``[[overrides]]`` entries."""
    document = tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))
    entries = document.get("profile", {}).get("default", {}).get("overrides", [])
    return [entry for entry in entries if isinstance(entry, dict)]


def _group_entries(name: str) -> list[dict[str, object]]:
    """Return the overrides assigning a test to ``name``."""
    return [entry for entry in _overrides() if entry.get("test-group") == name]


def _boundary_allowance() -> float:
    """Return the per-test ceiling that applies to the boundary binary."""
    entries = [
        entry
        for entry in _overrides()
        if entry.get("filter") == f"binary({BINARY})"
    ]
    assert len(entries) == 1, (
        f"expected one timeout override for binary({BINARY}), found {len(entries)}"
    )
    assert "slow-timeout" in entries[0], (
        f"the timeout override for binary({BINARY}) has no slow-timeout"
    )
    return budget_of(
        "profile.default.overrides[binary(boundary)]", entries[0]["slow-timeout"]
    )


def test_the_group_is_serial() -> None:
    """A group with two slots serializes nothing.

    Scenario: two packaged contracts run side by side. Invariant: the
    group grants one thread, which is what makes the shared target
    directory safe between processes.
    """
    document = tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))
    groups = document["test-groups"]
    assert groups[GROUP]["max-threads"] == 1, (
        f"the {GROUP} group must serialize its members; it grants "
        f"{groups[GROUP]['max-threads']} threads"
    )


def test_the_group_names_a_binary_that_exists() -> None:
    """A filter naming a binary Cargo does not build serializes nothing.

    Scenario: the ``[[test]]`` target is renamed or its ``name`` changes.
    Invariant: the group's filter selects exactly that binary, and the
    manifest still declares it. A typo here would leave every packaged
    contract unserialized while this file read as sound.
    """
    manifest = tomllib.loads(MANIFEST.read_text(encoding="utf-8"))
    declared = [entry["name"] for entry in manifest.get("test", [])]
    assert BINARY in declared, (
        f"{MANIFEST} declares no [[test]] target named {BINARY!r}; it "
        f"declares {declared}, so the nextest group selects nothing"
    )

    entries = _group_entries(GROUP)
    assert len(entries) == 1, (
        f"expected one override assigning tests to the {GROUP} group, found "
        f"{len(entries)}"
    )
    selected = entries[0].get("filter")
    assert selected == f"binary({BINARY})", (
        f"the {GROUP} group must select exactly binary({BINARY}); it selects "
        f"{selected!r}. A filter naming another binary leaves the packaged "
        f"contracts running in parallel"
    )


def test_no_other_override_assigns_the_boundary_binary() -> None:
    """Two groups for one binary is a configuration nextest cannot honour.

    Scenario: the binary is also added to a cache group for an unrelated
    reason. Invariant: it belongs to exactly one group, so which one
    applies is not decided by file order.
    """
    assigning = [
        entry
        for entry in _overrides()
        if f"binary({BINARY})" in str(entry.get("filter"))
    ]
    assert len(assigning) == 1, (
        f"binary({BINARY}) is named by {len(assigning)} overrides; exactly one "
        f"must assign its group so the effective group is not decided by "
        f"declaration order: {assigning}"
    )


def _single_match(
    pattern: str,
    source: str,
    what: str,
    path: pathlib.Path = PACKAGE_SET,
) -> re.Match[str]:
    """Return the one match of ``pattern``, or fail naming ``what``.

    A regex that matches nothing and one that matches twice are both a
    contract that cannot read what it means to check, so both are
    failures rather than a silently chosen first match.
    """
    matches = list(re.finditer(pattern, source))
    assert len(matches) == 1, (
        f"expected exactly one {what} in {path}, found {len(matches)}; "
        f"this contract cannot read the figure it is meant to check"
    )
    return matches[0]


def test_the_packaging_deadline_fits_inside_the_allowance() -> None:
    """A bound above the allowance never fires, so it names nothing.

    Scenario: a packaging invocation hangs on the package-cache lock.
    Invariant: each packaging invocation can reach its own deadline before
    nextest stops the test, so the failure names the invocation that hung
    rather than reporting a bare timeout.

    The deadline and the invocation count are both read from the fixture
    rather than restated, so the arithmetic cannot drift from the code
    that enforces it.
    """
    source = PACKAGE_SET.read_text(encoding="utf-8")
    deadline = int(
        _single_match(
            r"PACKAGE_DEADLINE:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)",
            source,
            "PACKAGE_DEADLINE declaration",
        )[1]
    )
    invocations = int(
        _single_match(
            r"PACKAGING_INVOCATIONS:\s*\[PackageInvocation;\s*(\d+)\]",
            source,
            "PACKAGING_INVOCATIONS array length",
        )[1]
    )

    allowance = _boundary_allowance()
    assert deadline * invocations < allowance, (
        f"each packaging invocation is bounded at {deadline}s and {invocations} "
        f"share one test, so they need {deadline * invocations}s of a "
        f"{allowance:.0f}s per-test allowance. A bound that does not fit is never "
        f"reached: nextest stops the test first and the error naming the hung "
        f"invocation is lost"
    )


def test_the_nested_cargo_deadline_fits_inside_the_allowance() -> None:
    """A child deadline must fire before nextest stops its parent test."""
    source = CARGO_ENVIRONMENT.read_text(encoding="utf-8")
    deadline = int(
        _single_match(
            r"CARGO_DEADLINE:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)",
            source,
            "CARGO_DEADLINE declaration",
            CARGO_ENVIRONMENT,
        )[1]
    )

    allowance = _boundary_allowance()
    assert deadline < allowance, (
        f"a nested Cargo invocation is bounded at {deadline}s, but binary({BINARY}) "
        f"has a {allowance:.0f}s per-test allowance. Nextest would stop the test "
        f"before the fixture reports which child exceeded its deadline"
    )
