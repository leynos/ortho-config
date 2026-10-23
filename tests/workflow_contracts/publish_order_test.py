"""Contract for the publish order in ``lading.toml``.

``lading publish`` packages and dry-run publishes each crate in the order
``[publish] order`` lists. That order is a claim about the workspace's
dependency graph, and it is invisible to every other gate: a crate listed
before something it depends on still builds locally, because Cargo resolves the
path dependency, and fails only when a publisher asks crates.io for a version
that does not exist yet.

So the order is checked against the graph rather than restated beside it. Two
properties are load-bearing:

- every published crate appears, and nothing unpublished does;
- a crate appears after everything it depends on.

The published set is derived from the manifests: a workspace member that does
not set ``publish = false`` is published. The edges are derived the same way,
from the ``path`` dependencies the manifests already declare, so a new internal
edge cannot be added without this contract reading it.

# Dev-dependency cycles

The workspace currently has none, so the ordered check exempts nothing. The
exemption is kept anyway, because a cycle here is a real hazard rather than a
hypothesis: until it was removed, the macros crate carried a dev dependency on
the library to test the macros it generates, which made the pair mutually
dependent and meant no order could put both first. That edge existed for no test
- every ``ortho_config::`` spelling in the macros crate is inside ``quote!``
token text or an ignored doc fence - but it still cost real behaviour:
verifying the packaged macros tarball pulls the patched library in through its
*normal* edge and duplicates the very package being verified, which Cargo
refuses as a lockfile collision. Keeping the exemption means a future cycle
still cannot fail the ordering check for a reason no order can fix. It is not
covered by a test of its own; the only cycle it ever had is the one described
here, and manufacturing one to test the exemption would put a fictional edge in
the graph this contract exists to read.

The exemption has to stay narrow. ``ortho_config`` has a dev dependency on
``test_helpers``, and that edge is *not* cyclic, so it is still ordered:
``test_helpers`` must be published before ``ortho_config``. That requirement is
easy to break by accident and invisible locally, because the workspace supplies
the path dependency either way; commit ``e1ca1a4c`` exists precisely to fix it.
A blanket "dev dependencies are not real edges" rule would have re-broken it.

Only internal edges are checked. An external crate's publication order is the
registry's problem, and ``cargo-orthohelp``'s version is its own rather than the
workspace's, so the file is read for names and edges rather than for a shared
version.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pathlib
import tomllib
import typing as typ

REPOSITORY_ROOT: typ.Final[pathlib.Path] = pathlib.Path(__file__).resolve().parents[2]
LADING_PATH: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "lading.toml"
WORKSPACE_MANIFEST: typ.Final[pathlib.Path] = REPOSITORY_ROOT / "Cargo.toml"

#: Dependency tables that describe a real build edge between members.
EDGE_TABLES: typ.Final[tuple[str, ...]] = (
    "dependencies",
    "dev-dependencies",
    "build-dependencies",
)


def _workspace_members() -> list[pathlib.Path]:
    """Return the manifest paths of every workspace member."""
    document = tomllib.loads(WORKSPACE_MANIFEST.read_text(encoding="utf-8"))
    patterns = document["workspace"]["members"]
    return [
        REPOSITORY_ROOT / pattern / "Cargo.toml"
        for pattern in patterns
        if (REPOSITORY_ROOT / pattern / "Cargo.toml").is_file()
    ]


def _manifest(manifest_path: pathlib.Path) -> dict[str, object]:
    """Parse one member manifest."""
    return tomllib.loads(manifest_path.read_text(encoding="utf-8"))


def _package_name(manifest: dict[str, object]) -> str:
    """Return the name a member publishes under."""
    return str(manifest["package"]["name"])  # type: ignore[index]


def _is_published(manifest: dict[str, object]) -> bool:
    """Return whether a member's manifest allows a registry publish.

    Cargo reads ``publish = false`` as "never publish" and an explicit list as
    "only these registries"; a missing key publishes anywhere. ``test_helpers``
    uses the list form, so a truthiness or ``!= []`` test would get both
    spellings backwards.
    """
    publish = (manifest["package"]).get("publish")  # type: ignore[index]
    return publish is not False and publish != []


def _path_dependencies(
    manifest_path: pathlib.Path,
    manifest: dict[str, object],
    by_directory: dict[pathlib.Path, str],
) -> set[str]:
    """Return the internal crates this manifest depends on by path.

    A ``path`` dependency is written relative to the manifest that declares it
    (``../ortho_l10n``), so it is resolved against that manifest's directory.
    Resolving against the process's working directory instead silently matches
    nothing, and an edge set that is always empty makes the ordering test pass
    without checking anything.
    """
    edges: set[str] = set()
    for table in EDGE_TABLES:
        for entry in (manifest.get(table) or {}).values():
            if not isinstance(entry, dict):
                continue
            declared = entry.get("path")
            if declared is None:
                continue
            resolved = (manifest_path.parent / str(declared)).resolve()
            name = by_directory.get(resolved)
            if name is not None:
                edges.add(name)
    return edges


def _published_crates() -> dict[str, set[str]]:
    """Return each published crate's internal dependency names."""
    manifests = {path: _manifest(path) for path in _workspace_members()}
    by_directory = {
        path.parent.resolve(): _package_name(manifest)
        for path, manifest in manifests.items()
    }
    return {
        _package_name(manifest): _path_dependencies(path, manifest, by_directory)
        for path, manifest in manifests.items()
        if _is_published(manifest)
    }


def _publish_order() -> list[str]:
    """Return the crate names ``lading.toml`` publishes, in order."""
    document = tomllib.loads(LADING_PATH.read_text(encoding="utf-8"))
    return list(document["publish"]["order"])


def _mutual_edges(edges: dict[str, set[str]]) -> set[frozenset[str]]:
    """Return the pairs that depend on each other, in either direction."""
    return {
        frozenset((name, dependency))
        for name, dependencies in edges.items()
        for dependency in dependencies
        if name in edges.get(dependency, set())
    }


def _ordering_violations(edges: dict[str, set[str]], order: list[str]) -> list[str]:
    """Return one message per edge the order publishes backwards."""
    position = {name: index for index, name in enumerate(order)}
    exempt = _mutual_edges(edges)
    return [
        f"{name} (position {position[name]}) depends on {dependency} "
        f"(position {position[dependency]})"
        for name, dependencies in edges.items()
        for dependency in dependencies
        if dependency in position
        and position[dependency] > position[name]
        and frozenset((name, dependency)) not in exempt
    ]


def test_the_order_names_every_published_crate() -> None:
    """A crate missing from the order is never published at all.

    Scenario: a member drops ``publish = false``, or a new crate is published.
    Invariant: the order is exactly the set of publishable members, so the dry
    run covers what a release would ship and nothing else.
    """
    published = set(_published_crates())
    ordered = _publish_order()

    assert set(ordered) == published, (
        "lading.toml publishes "
        f"{sorted(set(ordered))} but the workspace's publishable members are "
        f"{sorted(published)}; missing: {sorted(published - set(ordered))}, "
        f"unexpected: {sorted(set(ordered) - published)}"
    )
    assert len(ordered) == len(set(ordered)), (
        f"lading.toml lists a crate twice: {ordered}"
    )


def test_every_crate_follows_its_dependencies() -> None:
    """A dependency published later cannot be resolved when it is needed.

    Scenario: a new internal edge is added, or the order is edited by hand.
    Invariant: for every edge ``a -> b`` outside a dev-dependency cycle, ``b``
    is published before ``a``.
    """
    violations = _ordering_violations(_published_crates(), _publish_order())

    assert not violations, (
        "lading.toml publishes a crate before something it depends on, so the "
        "dependency is not on the registry when it is needed:\n  "
        + "\n  ".join(sorted(violations))
    )


def test_a_known_acyclic_dev_edge_still_constrains_the_order() -> None:
    """The cycle exemption must not swallow a dev edge that has no return path.

    Scenario: someone widens the exemption to skip dev dependencies wholesale.
    Invariant: the test helper crate is published before ``ortho_config``, which
    its dev dependency requires and which ``e1ca1a4c`` was written to restore.
    """
    edges = _published_crates()
    order = _publish_order()
    helper = "ortho_config_test_helpers"

    assert helper in edges["ortho_config"], (
        "ortho_config no longer declares the test helper as a dependency, so "
        "this guard would pass without checking the edge it exists for"
    )
    assert not _mutual_edges(edges).intersection({frozenset(("ortho_config", helper))}), (
        f"{helper} and ortho_config must not have become mutually dependent"
    )
    assert order.index(helper) < order.index("ortho_config")


def test_every_published_crate_is_reachable_in_the_order() -> None:
    """An unpublished crate named in the order would fail the dry run.

    Scenario: a crate is added to the order while still marked unpublished.
    Invariant: the order names no member that ``publish = false`` excludes,
    which is the other half of the set equality stated per-crate here so a
    failure names the offending crate rather than only a set difference.
    """
    manifests = [
        _manifest(path) for path in _workspace_members()
    ]
    unpublished = {
        _package_name(manifest)
        for manifest in manifests
        if not _is_published(manifest)
    }
    ordered = set(_publish_order())

    assert not (ordered & unpublished), (
        "lading.toml publishes crates marked publish = false: "
        f"{sorted(ordered & unpublished)}"
    )
