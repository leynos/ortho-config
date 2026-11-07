"""Contract for the serialized ``boundary`` packaging group.

The boundary contracts materialize their packaged crates once per
*process*, and nextest runs each test in its own process. Several of them
therefore run ``cargo package`` and unpack the same target directory side
by side, and Cargo's own lock does not hold readers out while a package
root is removed and unpacked. One process can replace a root another is
reading, which shows up as an intermittent failure in a suite whose whole
purpose is to be the evidence for a published graph.

So ``.config/nextest.toml`` puts ``binary(boundary)`` in a single-threaded
group of its own, and the fixture's child deadlines are sized to fit
inside the allowance that group runs under. Both halves are load-bearing
and neither is visible from the other file:

- a group that names a binary which does not exist serializes nothing,
  and reads exactly like one that works;
- a deadline longer than the per-test allowance can never fire, so the
  error naming the invocation that hung is never printed.

The command deadlines and invocation count are read from the sources, and the
effective allowance is resolved in file order, so the arithmetic and the code
that enforces it cannot drift apart. See "Nextest test-group serialization" in
``docs/developers-guide.md``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import tomllib
import typing as typ

from nextest_precedence import effective_allowances, selected_by_universe
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
ROOT_CHECKS_MARGIN_SECONDS: typ.Final[int] = 120


def _overrides() -> list[dict[str, object]]:
    """Return the default profile's ``[[overrides]]`` entries."""
    document = tomllib.loads(NEXTEST_CONFIG.read_text(encoding="utf-8"))
    entries = document.get("profile", {}).get("default", {}).get("overrides", [])
    return [entry for entry in entries if isinstance(entry, dict)]


def _group_entries(name: str) -> list[dict[str, object]]:
    """Return the overrides assigning a test to ``name``."""
    return [entry for entry in _overrides() if entry.get("test-group") == name]


def test_the_group_is_serial() -> None:
    """A group with two slots serializes nothing.

    Scenario: two packaged contracts run side by side. Invariant: the
    group grants one thread, which is what makes the workspace target's shared
    package directory safe between processes.
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


def _single_match(pattern: str, source: str, what: str) -> re.Match[str]:
    """Return the one match of ``pattern``, or fail naming ``what``.

    A regex that matches nothing and one that matches twice are both a
    contract that cannot read what it means to check, so both are
    failures rather than a silently chosen first match.
    """
    matches = list(re.finditer(pattern, source))
    assert len(matches) == 1, (
        f"expected exactly one {what} in {PACKAGE_SET}, found {len(matches)}; "
        f"this contract cannot read the figure it is meant to check"
    )
    return matches[0]


def test_the_packaging_deadline_fits_inside_the_allowance() -> None:
    """A bound above the allowance never fires, so it names nothing.

    Scenario: a packaging invocation hangs on the package-cache lock.
    Invariant: each packaging invocation can reach its own deadline before
    nextest stops the test, so the failure names the invocation that hung
    rather than reporting a bare timeout.

    The command deadlines and invocation count are read from the fixtures,
    and the allowance is resolved for this binary in file order. That keeps
    the arithmetic tied to the bound the runner actually applies.
    """
    package_source = PACKAGE_SET.read_text(encoding="utf-8")
    package_deadline = int(
        _single_match(
            r"PACKAGE_DEADLINE:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)",
            package_source,
            "PACKAGE_DEADLINE declaration",
        )[1]
    )
    invocations = int(
        _single_match(
            r"PACKAGING_INVOCATIONS:\s*\[PackageInvocation;\s*(\d+)\]",
            package_source,
            "PACKAGING_INVOCATIONS array length",
        )[1]
    )
    cargo_environment = (
        REPOSITORY_ROOT / CRATE / "tests" / "boundary" / "fixture_cargo" / "environment.rs"
    ).read_text(encoding="utf-8")
    cargo_deadline = int(
        _single_match(
            r"CARGO_DEADLINE:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)",
            cargo_environment,
            "CARGO_DEADLINE declaration",
        )[1]
    )

    config_text = NEXTEST_CONFIG.read_text(encoding="utf-8")
    tier = effective_allowances(
        config_text,
        [BINARY],
        reader=selected_by_universe([BINARY]),
    )[BINARY]
    assert not tier.is_unresolved, f"the {BINARY} allowance could not be resolved"
    package_budget = package_deadline * invocations
    assert package_budget + ROOT_CHECKS_MARGIN_SECONDS <= tier.allowance, (
        f"{invocations} packaging invocations and the "
        f"{ROOT_CHECKS_MARGIN_SECONDS}s root-check margin need "
        f"{package_budget + ROOT_CHECKS_MARGIN_SECONDS}s, but nextest resolves "
        f"binary({BINARY}) to {tier.allowance:.0f}s"
    )
    assert cargo_deadline + ROOT_CHECKS_MARGIN_SECONDS <= tier.allowance, (
        f"a consumer Cargo command and the root-check margin need "
        f"{cargo_deadline + ROOT_CHECKS_MARGIN_SECONDS}s, but nextest resolves "
        f"binary({BINARY}) to {tier.allowance:.0f}s"
    )
