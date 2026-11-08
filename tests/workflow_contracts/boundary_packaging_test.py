"""Contract for the serialized ``boundary`` packaging group.

The boundary contracts materialize their packaged crates once per
*process*, and nextest runs each test in its own process. Several of them
therefore run ``cargo package`` and unpack the same target directory side
by side, and Cargo's own lock does not hold readers out while a package
root is removed and unpacked. One process can replace a root another is
reading, which shows up as an intermittent failure in a suite whose whole
purpose is to be the evidence for a published graph.

So ``.config/nextest.toml`` puts ``binary(boundary)`` in a single-threaded
group of its own, and gives that override its own per-test allowance. The
fixture's packaging and nested-Cargo deadlines are sized to fit inside that
allowance. Both halves are load-bearing and neither is visible from the
other file:

- a group that names a binary which does not exist serializes nothing,
  and reads exactly like one that works;
- a deadline longer than the per-test allowance can never fire, so the
  error naming the invocation that hung is never printed;
- an allowance that does not exceed the profile's own buys no headroom,
  which is how six packaged contracts spent their 240 s bound waiting for
  compile admission and reported only the deadline.

The deadlines and the invocation count are read from the sources rather
than restated here, and the allowance is read from the boundary override
rather than from the profile, so the arithmetic and the code that enforces
it cannot drift apart. See "Nextest test-group serialization" in
``docs/developers-guide.md``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import tomllib
import typing as typ

from nextest_budgets import override_allowances, profile_allowance
from timeout_budgets import NEXTEST_CONFIG

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]

#: The binary whose tests share one packaging target directory.
BINARY: typ.Final[str] = "boundary"

#: The test group and the crate it belongs to.
GROUP: typ.Final[str] = "boundary_packaging"
CRATE: typ.Final[str] = "ortho_l10n"

#: The manifest declaring the binary, and the fixtures declaring its deadlines.
MANIFEST: typ.Final[pathlib.Path] = REPOSITORY_ROOT / CRATE / "Cargo.toml"
PACKAGE_SET: typ.Final[pathlib.Path] = (
    REPOSITORY_ROOT / CRATE / "tests" / "boundary" / "package_set" / "mod.rs"
)
ENVIRONMENT: typ.Final[pathlib.Path] = (
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
    pattern: str, source: str, what: str, where: pathlib.Path
) -> re.Match[str]:
    """Return the one match of ``pattern``, or fail naming ``what``.

    A regex that matches nothing and one that matches twice are both a
    contract that cannot read what it means to check, so both are
    failures rather than a silently chosen first match.
    """
    matches = list(re.finditer(pattern, source))
    assert len(matches) == 1, (
        f"expected exactly one {what} in {where}, found {len(matches)}; "
        f"this contract cannot read the figure it is meant to check"
    )
    return matches[0]


def _seconds_declaration(path: pathlib.Path, name: str) -> int:
    """Read a ``name: Duration = Duration::from_secs(N)`` declaration."""
    source = path.read_text(encoding="utf-8")
    pattern = rf"{name}:\s*Duration\s*=\s*Duration::from_secs\((\d+)\)"
    return int(_single_match(pattern, source, f"{name} declaration", path)[1])


def _boundary_allowance() -> float:
    """Return the per-test allowance the boundary override grants, in seconds.

    Read from the override rather than from the profile: the profile's own
    allowance applies only to tests no override matches, so comparing the
    deadlines against it would check the wrong figure.
    """
    config = NEXTEST_CONFIG.read_text(encoding="utf-8")
    allowances = [
        budget
        for _path, selector, budget in override_allowances(config)
        if selector is not None and f"binary({BINARY})" in selector
    ]
    assert len(allowances) == 1, (
        f"expected exactly one override carrying a slow-timeout for "
        f"binary({BINARY}), found {len(allowances)}; without one the binary "
        f"runs on the profile's base allowance and the deadline arithmetic "
        f"below is checked against a figure no test uses"
    )
    return allowances[0]


def test_the_boundary_allowance_buys_headroom() -> None:
    """An override that restates the base allowance buys nothing.

    Scenario: the boundary override loses its ``slow-timeout``, or gains
    one equal to the profile's. Invariant: the boundary binary runs on a
    longer allowance than the rest of the suite, which is what the raised
    packaging and nested-Cargo bounds rely on.
    """
    base = profile_allowance(NEXTEST_CONFIG.read_text(encoding="utf-8"))
    allowance = _boundary_allowance()
    assert allowance > base, (
        f"the boundary override grants {allowance:.0f}s, no more than the "
        f"{base:.0f}s base allowance; the raised deadlines then never fire and "
        f"the error naming the hung invocation is lost"
    )


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
    deadline = _seconds_declaration(PACKAGE_SET, "PACKAGE_DEADLINE")
    invocations = int(
        _single_match(
            r"PACKAGING_INVOCATIONS:\s*\[PackageInvocation;\s*(\d+)\]",
            PACKAGE_SET.read_text(encoding="utf-8"),
            "PACKAGING_INVOCATIONS array length",
            PACKAGE_SET,
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


def test_a_consumer_resolve_and_build_fit_inside_the_allowance() -> None:
    """The two nested Cargo calls one consumer contract makes must fit.

    Scenario: ``workspace_runtime_consumer_has_no_facade_closure`` resolves
    the consumer and then builds it. Invariant: both bounds can expire
    before nextest stops the test, so either hung call is named rather than
    replaced by a bare timeout. A consumer contract that makes more calls
    cannot be covered by any per-call bound; its first pair is what this
    guarantees.
    """
    deadline = _seconds_declaration(ENVIRONMENT, "CARGO_DEADLINE")
    allowance = _boundary_allowance()
    assert deadline * 2 < allowance, (
        f"a consumer contract resolves and builds under one test, each call "
        f"bounded at {deadline}s, so the pair needs {deadline * 2}s of a "
        f"{allowance:.0f}s allowance. A bound that does not fit is never "
        f"reached: nextest stops the test first"
    )
