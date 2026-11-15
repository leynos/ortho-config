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

A per-call cap also cannot bound a contract that makes several calls: a
contract making eight calls at a 440 s cap each can spend 3,520 s of a
960 s allowance, and nextest then stops the test before any later call
reports its own deadline. The fixture therefore shares one
``SCENARIO_ALLOWANCE`` across every call in a contract, and these
contracts check that the allowance is inside the nextest allowance, that
every cap fits inside it, and that the complete sequence is long enough
that the cap alone would not have bounded it.

The deadlines, the invocation count and the consumer shape count are read
from the sources rather than restated here, and the allowance is read from
the boundary override rather than from the profile, so the arithmetic and
the code that enforces it cannot drift apart. See "Nextest test-group
serialization" in ``docs/developers-guide.md``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import re
import tomllib
import typing as typ

from nextest_budgets import global_timeout, override_allowances, profile_allowance
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
BUDGET: typ.Final[pathlib.Path] = (
    REPOSITORY_ROOT / CRATE / "tests" / "boundary" / "fixture_cargo" / "budget.rs"
)
CONSUMER: typ.Final[pathlib.Path] = (
    REPOSITORY_ROOT / CRATE / "tests" / "boundary" / "consumer" / "mod.rs"
)
PROCESS: typ.Final[pathlib.Path] = (
    REPOSITORY_ROOT / CRATE / "tests" / "boundary" / "fixture_cargo" / "process.rs"
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


def _packaging_invocations() -> int:
    """Read the number of packaging invocations one test makes."""
    return int(
        _single_match(
            r"PACKAGING_INVOCATIONS:\s*\[PackageInvocation;\s*(\d+)\]",
            PACKAGE_SET.read_text(encoding="utf-8"),
            "PACKAGING_INVOCATIONS array length",
            PACKAGE_SET,
        )[1]
    )


def _consumer_shapes() -> int:
    """Read the number of consumer shapes the packaged contract builds."""
    return int(
        _single_match(
            r"ALL:\s*\[Self;\s*(\d+)\]",
            CONSUMER.read_text(encoding="utf-8"),
            "Dependency::ALL array length",
            CONSUMER,
        )[1]
    )


def test_the_unsupported_platform_is_rejected_at_compile_time() -> None:
    """A target with neither process group nor Job Object cannot supervise.

    Scenario: the runner is built for a platform that offers neither tree
    primitive. Invariant: the build is refused at compile time, and both
    supported platforms keep their own wrapper. ``trybuild`` cannot express
    this on any host this suite runs on: it builds for the host target, and
    ``rust-toolchain.toml`` installs only ``x86_64-unknown-linux-gnu`` and
    ``x86_64-pc-windows-msvc``, both of which are supported. A fixture that
    re-declared the guard would compile the fixture rather than this module,
    and a fixture that included the module would need a target whose standard
    library is not installed. Reading the rejection, its exact diagnostic and
    its position inside ``spawn_owned`` is the equivalent check that runs on
    every leg. A change that removed the rejection, widened it to a supported
    platform, or moved it away from the spawn it protects fails here.
    """
    source = PROCESS.read_text(encoding="utf-8")
    guard = _single_match(
        r"#\[cfg\(not\(any\(unix, windows\)\)\)\]\s*compile_error!"
        r"\(\"process-tree supervision needs a Unix process group or a "
        r"Windows Job Object\"\);",
        source,
        "unsupported-platform compile_error",
        PROCESS,
    )
    for platform, wrapper in (
        ("unix", "ProcessGroup::leader()"),
        ("windows", "JobObject"),
    ):
        _single_match(
            rf"#\[cfg\({platform}\)\]\s*wrapped\.wrap\({re.escape(wrapper)}\)",
            source,
            f"{platform} process-tree wrapper",
            PROCESS,
        )
    # The rejection is only load-bearing where the tree is created. A guard
    # that drifted outside `spawn_owned`, or below its spawn call, would read
    # as present here while the unsupported build reached the wrapper.
    spawn_at = source.find("fn spawn_owned(")
    assert spawn_at >= 0, f"no spawn_owned in {PROCESS}"
    assert spawn_at < guard.start(), (
        f"the unsupported-platform guard must sit inside spawn_owned in {PROCESS}"
    )
    spawn_call = source.find(".spawn()", spawn_at)
    assert spawn_call >= 0, f"no .spawn() call after spawn_owned in {PROCESS}"
    assert guard.start() < spawn_call, (
        f"the guard must precede the spawn call it protects in {PROCESS}"
    )


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
    invocations = _packaging_invocations()

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


def test_the_shared_allowance_is_inside_the_nextest_allowance() -> None:
    """A shared allowance above the test's own never fires.

    Scenario: a contract's earlier calls consume the whole allowance.
    Invariant: the shared budget expires while nextest is still waiting, so
    the error names the allowance and the call that could not start rather
    than being replaced by nextest's bare timeout.
    """
    config = NEXTEST_CONFIG.read_text(encoding="utf-8")
    allowance = _boundary_allowance()
    scenario = _seconds_declaration(BUDGET, "SCENARIO_ALLOWANCE")
    assert scenario < allowance, (
        f"the shared allowance is {scenario}s and the boundary override grants "
        f"{allowance:.0f}s per test; a shared allowance that does not expire "
        f"first is never reported"
    )
    whole_run = global_timeout(config)
    assert whole_run is not None, "nextest default profile must set global-timeout"
    assert scenario < whole_run, (
        f"the shared allowance is {scenario}s and the whole run is bounded at "
        f"{whole_run:.0f}s; one contract's shared budget must fit inside the "
        f"run that contains it"
    )


def test_each_per_call_cap_fits_inside_the_shared_allowance() -> None:
    """A cap longer than the shared allowance is decided by the wrong bound.

    Scenario: one nested Cargo call hangs. Invariant: its own cap expires
    before the shared allowance, so the diagnostic names the invocation and
    its cap rather than blaming the contract's allowance.
    """
    scenario = _seconds_declaration(BUDGET, "SCENARIO_ALLOWANCE")
    for path, name in ((ENVIRONMENT, "CARGO_DEADLINE"), (PACKAGE_SET, "PACKAGE_DEADLINE")):
        cap = _seconds_declaration(path, name)
        assert cap < scenario, (
            f"{name} is {cap}s and the shared allowance is {scenario}s; the "
            f"allowance would expire first and the error would not name the "
            f"invocation whose own cap was exceeded"
        )


def test_the_shared_allowance_bounds_the_complete_sequence() -> None:
    """Per-call caps cannot bound a contract that makes several calls.

    The packaged consumer contract packages the set and then builds and
    resolves every consumer shape. Each call has a cap that fits inside the
    test on its own, but the calls together may spend far more than the
    per-test allowance, so the *shared* allowance is what stops a later call
    from being killed by nextest before it can report. If the complete
    sequence were already shorter than the allowance, the shared budget
    would be untested by this contract and this check would be vacuous.
    """
    scenario = _seconds_declaration(BUDGET, "SCENARIO_ALLOWANCE")
    packaging = _seconds_declaration(PACKAGE_SET, "PACKAGE_DEADLINE")
    consumer = _seconds_declaration(ENVIRONMENT, "CARGO_DEADLINE")
    invocations = _packaging_invocations()
    # Every shape both builds its consumer and resolves its metadata.
    calls = invocations + 2 * _consumer_shapes()
    worst_case = calls * max(packaging, consumer)
    assert worst_case > scenario, (
        f"the packaged consumer contract makes {calls} bounded calls of at "
        f"most {max(packaging, consumer)}s each, which is {worst_case}s "
        f"against a {scenario}s shared allowance; if the caps alone already "
        f"fit, this contract does not exercise the shared budget"
    )
