"""Generated models for child-cargo filter and module-graph readings.

The neighbouring tests pin individual failure shapes. These properties vary
override order and marker placement, then compare the readers with small
models that do not call the resolver or module walker being checked.
"""

from __future__ import annotations

import pathlib
import tempfile
import typing as typ

from child_cargo_source_markers import CARGO_MARKERS, marker_sources
from hypothesis import given, settings
from hypothesis import strategies as st
from nextest_precedence import effective_allowances, selected_by_universe
from rust_module_graph import sources_reached


class OverrideCase(typ.NamedTuple):
    """One generated nextest override and its modelled selection shape."""

    expression: str
    selected_names: frozenset[str] | None
    allowance: str | None
    is_platform_gated: bool = False


_CORE_OVERRIDES: typ.Final[tuple[OverrideCase, ...]] = (
    OverrideCase("binary(a)", frozenset({"a"}), "raised"),
    OverrideCase("binary(b)", frozenset({"b"}), "base"),
    OverrideCase("binary(a)", frozenset({"a"}), None),
    OverrideCase("binary(a)", frozenset({"a"}), "base", True),
    OverrideCase("test(slow)", None, "raised"),
    OverrideCase("all()", None, "base"),
    OverrideCase("binary(a) & test(slow)", None, "base"),
    OverrideCase("binary_id(package::a)", frozenset({"a"}), "raised"),
)
_EXTRA_OVERRIDES: typ.Final[tuple[OverrideCase, ...]] = (
    OverrideCase("binary(a)", frozenset({"a"}), "base"),
    OverrideCase("binary(b)", frozenset({"b"}), "raised"),
    OverrideCase("binary_id(other_package::b)", frozenset({"b"}), "base"),
    OverrideCase("test(slow)", None, "base"),
    OverrideCase("binary(a) & test(slow)", None, "raised"),
    OverrideCase("binary(a)", frozenset({"a"}), None),
)


@st.composite
def _override_sequences(draw: st.DrawFn) -> list[OverrideCase]:
    """Draw mandatory filter shapes in arbitrary order with bounded extras."""
    extras = draw(st.lists(st.sampled_from(_EXTRA_OVERRIDES), max_size=4))
    return list(draw(st.permutations((*_CORE_OVERRIDES, *extras))))


def _nextest_config(entries: list[OverrideCase]) -> str:
    """Serialize model entries in the profile and override shape nextest reads."""
    lines = [
        "[profile.default]",
        'slow-timeout = { period = "60s", terminate-after = 10 }',
    ]
    for entry in entries:
        lines.extend(
            [
                "",
                "[[profile.default.overrides]]",
                f'filter = "{entry.expression}"',
            ]
        )
        if entry.is_platform_gated:
            lines.append('platform = "cfg(unix)"')
        if entry.allowance == "base":
            lines.append('slow-timeout = { period = "60s", terminate-after = 10 }')
        elif entry.allowance == "raised":
            lines.append('slow-timeout = { period = "120s", terminate-after = 8 }')
        else:
            lines.append('test-group = "shared"')
    return "\n".join(lines) + "\n"


def _reference_tier(
    name: str, entries: list[OverrideCase]
) -> tuple[float, str, bool, bool]:
    """Resolve one candidate from generated facts, without parsing its TOML."""
    allowance = 600.0
    path = "profile.default"
    unresolved = False
    for index, entry in enumerate(entries):
        if entry.allowance is None:
            continue
        selected = None if entry.selected_names is None else name in entry.selected_names
        if selected is None or (entry.is_platform_gated and selected):
            unresolved = True
        elif selected:
            allowance = 600.0 if entry.allowance == "base" else 960.0
            path = f"profile.default.overrides[{index}]"
            return allowance, path, allowance > 600.0, unresolved
    return allowance, path, False, unresolved


@settings(max_examples=100, deadline=None)
@given(entries=_override_sequences())
def test_effective_allowances_match_an_independent_ordered_model(
    entries: list[OverrideCase],
) -> None:
    """First selected setting wins; earlier uncertainty remains visible."""
    names = ["a", "b"]
    config = _nextest_config(entries)
    resolved = effective_allowances(config, names, reader=selected_by_universe(names))
    reversed_result = effective_allowances(
        config, list(reversed(names)), reader=selected_by_universe(reversed(names))
    )
    expected = {name: _reference_tier(name, entries) for name in names}
    actual = {
        name: (tier.allowance, tier.path, tier.is_above_base, tier.is_unresolved)
        for name, tier in resolved.items()
    }
    assert actual == expected
    assert reversed_result == resolved, "candidate order must not alter a tier"


class ModuleGraph(typ.NamedTuple):
    """Flags and marker sources for a bounded, acyclic test-source graph."""

    branches: tuple[bool, bool]
    branch_children: tuple[bool, bool]
    shared_helpers: tuple[bool, bool]
    inline_children: tuple[bool, bool]
    markers: dict[str, str | None]


_SOURCE_KEYS: typ.Final[tuple[str, ...]] = (
    "root_0",
    "root_1",
    "branch_0",
    "branch_1",
    "branch_leaf_0",
    "branch_leaf_1",
    "shared",
    "inline_leaf_0",
    "inline_leaf_1",
    "orphan",
)
_MARKER_TEXT: typ.Final[tuple[str, ...]] = (
    'Command::new("cargo")',
    "cargo_orthohelp_exe",
    "prepare_cargo_command_for_target_before",
)


@st.composite
def _module_graphs(draw: st.DrawFn) -> ModuleGraph:
    """Draw two roots sharing optional modules with independent marker text."""
    pair = st.tuples(st.booleans(), st.booleans())
    markers = draw(
        st.dictionaries(
            keys=st.sampled_from(_SOURCE_KEYS),
            values=st.one_of(st.none(), st.sampled_from(_MARKER_TEXT)),
            min_size=len(_SOURCE_KEYS),
            max_size=len(_SOURCE_KEYS),
        )
    )
    return ModuleGraph(
        branches=draw(pair),
        branch_children=draw(pair),
        shared_helpers=draw(pair),
        inline_children=draw(pair),
        markers=markers,
    )


def _with_marker(source: str, marker: str | None) -> str:
    """Put one selected marker into a source without changing its topology."""
    statement = f"let _ = {marker};" if marker else "let _ = 1;"
    return f"fn exercise() {{ {statement} }}\n{source}"


def _write_module_graph(tests: pathlib.Path, graph: ModuleGraph) -> None:
    """Write every possible node so declarations alone control reachability."""
    for index in range(2):
        root_content = ""
        if graph.branches[index]:
            root_content += f"mod branch_{index};\n"
        if graph.shared_helpers[index]:
            root_content += f'#[path = "support/shared.rs"]\nmod shared_alias_{index};\n'
        if graph.inline_children[index]:
            root_content += f"mod inline_{index} {{\n    mod inline_leaf_{index};\n}}\n"
        (tests / f"binary_{index}.rs").write_text(
            _with_marker(root_content, graph.markers[f"root_{index}"]),
            encoding="utf-8",
        )
        branch_content = (
            f"mod branch_leaf_{index};\n" if graph.branch_children[index] else ""
        )
        (tests / f"branch_{index}.rs").write_text(
            _with_marker(branch_content, graph.markers[f"branch_{index}"]),
            encoding="utf-8",
        )
        branch_dir = tests / f"branch_{index}"
        branch_dir.mkdir(exist_ok=True)
        (branch_dir / f"branch_leaf_{index}.rs").write_text(
            _with_marker("", graph.markers[f"branch_leaf_{index}"]),
            encoding="utf-8",
        )
        inline_dir = tests / f"inline_{index}"
        inline_dir.mkdir(exist_ok=True)
        (inline_dir / f"inline_leaf_{index}.rs").write_text(
            _with_marker("", graph.markers[f"inline_leaf_{index}"]),
            encoding="utf-8",
        )
    support_dir = tests / "support"
    support_dir.mkdir(exist_ok=True)
    (support_dir / "shared.rs").write_text(
        _with_marker("", graph.markers["shared"]), encoding="utf-8"
    )
    (tests / "orphan.rs").write_text(
        _with_marker("", graph.markers["orphan"]), encoding="utf-8"
    )


def _expected_sources(
    tests: pathlib.Path, graph: ModuleGraph, index: int
) -> set[pathlib.Path]:
    """Model the files reachable through the generated declarations."""
    reached = {tests / f"binary_{index}.rs"}
    if graph.branches[index]:
        reached.add(tests / f"branch_{index}.rs")
        if graph.branch_children[index]:
            reached.add(tests / f"branch_{index}" / f"branch_leaf_{index}.rs")
    if graph.shared_helpers[index]:
        reached.add(tests / "support" / "shared.rs")
    if graph.inline_children[index]:
        reached.add(tests / f"inline_{index}" / f"inline_leaf_{index}.rs")
    return reached


def _source_key(tests: pathlib.Path, source: pathlib.Path) -> str:
    """Map a generated path back to the independent marker assignment."""
    relative = source.relative_to(tests).as_posix()
    aliases = {
        "binary_0.rs": "root_0",
        "binary_1.rs": "root_1",
        "branch_0.rs": "branch_0",
        "branch_1.rs": "branch_1",
        "branch_0/branch_leaf_0.rs": "branch_leaf_0",
        "branch_1/branch_leaf_1.rs": "branch_leaf_1",
        "support/shared.rs": "shared",
        "inline_0/inline_leaf_0.rs": "inline_leaf_0",
        "inline_1/inline_leaf_1.rs": "inline_leaf_1",
        "orphan.rs": "orphan",
    }
    return aliases[relative]


@settings(max_examples=80, deadline=None)
@given(graph=_module_graphs())
def test_module_walk_matches_an_independent_graph_and_marker_model(
    graph: ModuleGraph,
) -> None:
    """Reachable markers count for each user; orphan markers count for none."""
    with tempfile.TemporaryDirectory() as temporary:
        tests = pathlib.Path(temporary) / "tests"
        tests.mkdir()
        _write_module_graph(tests, graph)
        for index in range(2):
            root = tests / f"binary_{index}.rs"
            expected_sources = _expected_sources(tests, graph, index)
            actual_sources = sources_reached(root)
            assert set(actual_sources) == expected_sources
            expected_markers: dict[str, list[pathlib.Path]] = {}
            for position, (label, _) in enumerate(CARGO_MARKERS):
                marker_text = _MARKER_TEXT[position]
                marked_sources = [
                    source
                    for source in expected_sources
                    if graph.markers[_source_key(tests, source)] == marker_text
                ]
                if marked_sources:
                    expected_markers[label] = sorted(marked_sources)
            actual_markers = {
                label: sorted(sources) for label, sources in marker_sources(root).items()
            }
            assert actual_markers == expected_markers
