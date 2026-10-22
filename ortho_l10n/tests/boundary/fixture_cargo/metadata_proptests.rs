//! Property tests for the transitive closure over Cargo's resolve graph.
//!
//! [`Metadata::reachable`] answers "what does a consumer of this crate
//! compile", and every boundary contract rests on that answer. Its arithmetic
//! therefore has to hold for graph shapes no fixture happens to build today:
//! cycles, diamonds, parallel edges, and packages reached by several routes at
//! once.
//!
//! The oracle is deliberately not the implementation restated. The
//! implementation walks ids from a `BTreeSet` frontier; the oracle is the
//! textbook least fixed point, which unions one hop of successors into the
//! answer until nothing grows. A frontier that forgot to mark a node seen, or
//! a closure that stopped after one hop, agrees with the other formulation
//! only on trees - so a disagreement here localises the defect.
//!
//! Graphs are generated as names and rendered into Cargo's id spelling, so
//! these cases also exercise the name-identity mapping that a path id (whose
//! fragment carries no name) forces [`Metadata`] to perform.

use super::Metadata;
use super::metadata::{MetadataPackage, Resolve, ResolveDependency, ResolveNode};
use proptest::collection::{btree_set, vec};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::collections::{BTreeMap, BTreeSet};

/// Cargo's id prefix, so a generated id cannot be mistaken for a name.
const ID_PREFIX: &str = "path+file:///generated#";

/// A generated package name. Short and lower-case so a failure reads plainly.
fn package_name() -> impl Strategy<Value = String> {
    "[a-z]{1,6}"
}

/// One directed edge, as `(source, target)` indices into the package list.
///
/// Indices rather than names keep this family away from dangling edges by
/// accident: it is about closure arithmetic, and an edge into an unknown id is
/// a separate, deliberately-constructed case.
fn edge(count: usize) -> impl Strategy<Value = (usize, usize)> {
    (0..count, 0..count)
}

/// A graph's abstract shape: which packages exist, and how they connect.
///
/// `names` and `edges` are always passed together - every reader needs both to
/// resolve an edge's indices back to packages - so they travel as one value
/// rather than as a recurring pair of arguments.
#[derive(Debug)]
struct Shape {
    /// Package names, unique and sorted.
    names: Vec<String>,
    /// Directed edges, as indices into `names`.
    edges: Vec<(usize, usize)>,
}

impl Shape {
    /// Group the edges by source, keeping one entry per distinct target set.
    ///
    /// Out-of-range indices are skipped rather than indexed: [`edge`] never
    /// produces them, and [`clippy::indexing_slicing`] is denied
    /// workspace-wide anyway.
    fn adjacency(&self) -> BTreeMap<&str, BTreeSet<&str>> {
        let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for (source_index, target_index) in &self.edges {
            let (Some(source), Some(target)) =
                (self.names.get(*source_index), self.names.get(*target_index))
            else {
                continue;
            };
            adjacency
                .entry(source.as_str())
                .or_default()
                .insert(target.as_str());
        }
        adjacency
    }
}

/// A generated graph: its shape, and the root to close over.
///
/// Names come from a set, so they are unique and sorted. That is a narrowing of
/// the documents Cargo actually emits, not a law about them: two *versions* of
/// one package resolve under distinct ids that share a name, and
/// `name_of_id`-based reachability has to traverse through both before
/// de-duplicating. These generated cases therefore cover graph *shape*, with
/// one id per name, and [`reachable_merges_two_ids_that_share_a_name`] covers
/// the shared-name case deliberately rather than by sampling.
///
/// The root is drawn from the same names, so a generated case is never an
/// accidental missing-root control.
fn graph() -> impl Strategy<Value = (Shape, String)> {
    btree_set(package_name(), 1..8).prop_flat_map(|generated| {
        let names: Vec<String> = generated.into_iter().collect();
        let count = names.len();
        let roots = names.clone();
        // The edge indices depend on the name count, so edges are drawn from
        // that count rather than from a fixed range.
        (vec(edge(count), 0..12), prop::sample::select(roots)).prop_map(move |(edges, root)| {
            (
                Shape {
                    names: names.clone(),
                    edges,
                },
                root,
            )
        })
    })
}

/// Render a graph into the `cargo metadata` shapes [`Metadata`] parses.
fn render(shape: &Shape) -> Metadata {
    let adjacency = shape.adjacency();
    let packages = shape
        .names
        .iter()
        .map(|name| package(name, &PackageId::of(name)))
        .collect();
    let nodes = shape
        .names
        .iter()
        .map(|name| {
            let targets: Vec<PackageId> = adjacency
                .get(name.as_str())
                .into_iter()
                .flatten()
                .map(|target| PackageId::of(target))
                .collect();
            node(&PackageId::of(name), &targets)
        })
        .collect();
    Metadata {
        packages,
        resolve: Resolve { nodes },
    }
}

/// A package id, in the `path+file:///…#name` spelling Cargo emits.
///
/// Deliberately distinct from a package *name*. [`Metadata`] walks ids and
/// reports names, and a version-split graph carries one name under several
/// ids, so the two are different keys. While both were spelled `&str` a name
/// could be passed where an id was wanted; that mistake surfaced only at run
/// time, as a missing-node error. The wrapper makes it a compile error.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PackageId(String);

impl PackageId {
    /// The id Cargo gives a package resolved from the local path `name`.
    fn of(name: &str) -> Self {
        Self(format!("{ID_PREFIX}{name}"))
    }
}

impl std::fmt::Display for PackageId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// One package entry carrying `name` under `id`.
///
/// Separate from [`render`] because the version-split case needs two packages
/// with one name, which a name-keyed builder cannot express.
fn package(name: &str, id: &PackageId) -> MetadataPackage {
    MetadataPackage {
        id: id.0.clone(),
        name: name.to_owned(),
        version: String::from("0.0.0"),
        source: None,
    }
}

/// A resolve dependency on `target`, with no dep-kind qualifiers.
fn dependency(target: &PackageId) -> ResolveDependency {
    ResolveDependency {
        pkg: target.0.clone(),
        dep_kinds: Vec::new(),
    }
}

/// A resolve node for `id` with edges onto each of `targets`.
///
/// `targets` are package *ids* rather than names: in a version-split graph a
/// name does not identify one node.
fn node(id: &PackageId, targets: &[PackageId]) -> ResolveNode {
    ResolveNode {
        id: id.0.clone(),
        deps: targets.iter().map(|target| dependency(target)).collect(),
    }
}

/// The textbook least fixed point over `edges`, as a set of names.
///
/// This is the oracle: start from the root alone and repeatedly add the
/// successors of everything already in the answer. No visited set and no
/// frontier - only the definition of reachability - so it cannot share the
/// implementation's bookkeeping defect.
fn reference_closure(shape: &Shape, root: &str) -> BTreeSet<String> {
    let mut reached = BTreeSet::from([root.to_owned()]);
    loop {
        let mut grown = reached.clone();
        for (source_index, target_index) in &shape.edges {
            let (Some(source), Some(target)) = (
                shape.names.get(*source_index),
                shape.names.get(*target_index),
            ) else {
                continue;
            };
            if reached.contains(source.as_str()) {
                grown.insert(target.clone());
            }
        }
        if grown == reached {
            return reached;
        }
        reached = grown;
    }
}

proptest! {
    /// The closure is exactly the oracle's fixed point, and it is sorted,
    /// de-duplicated, and includes the root that was asked about.
    ///
    /// Exact set equality is the assertion. A superset would hide a forbidden
    /// package a contract wrongly believes reachable; a subset would hide one
    /// it wrongly believes absent. Neither direction is a safe approximation
    /// for a boundary check.
    #[test]
    fn reachable_matches_the_least_fixed_point(
        (shape, root) in graph(),
    ) {
        let metadata = render(&shape);
        let actual = metadata
            .reachable(&root)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let expected = reference_closure(&shape, &root);
        let actual_names: BTreeSet<&str> = actual.iter().copied().collect();
        let expected_names: BTreeSet<&str> =
            expected.iter().map(String::as_str).collect();
        prop_assert_eq!(actual_names, expected_names);
        prop_assert!(actual.contains(&root.as_str()), "the root is always reachable");
        prop_assert!(
            actual.windows(2).all(|pair| pair.first() < pair.last()),
            "the closure is sorted and holds no duplicate: {actual:?}",
        );
    }

    /// An unknown root is an error, not an empty answer.
    ///
    /// An empty `Ok` would let a contract whose fixture failed to resolve pass
    /// its forbidden-package check, which is the vacuous success this suite
    /// exists to prevent. The name comes from the same alphabet as real names,
    /// so the case cannot be satisfied by a spelling mismatch.
    #[test]
    fn reachable_rejects_a_root_the_graph_lacks(
        (shape, root) in graph(),
        missing in package_name(),
    ) {
        prop_assume!(!shape.names.contains(&missing));
        let metadata = render(&shape);
        prop_assert!(metadata.reachable(&missing).is_err());
        prop_assert!(metadata.reachable(&root).is_ok());
    }

    /// An edge the graph cannot answer for is an error, whether the missing
    /// identity has no package entry or no resolve node.
    ///
    /// Cargo emits neither document, but each is a distinct way for a reader
    /// to answer a smaller closure than the truth with undiminished
    /// confidence. The two are generated as one boolean because they are two
    /// halves of one claim: an identity reached across an edge must resolve to
    /// a name. They must be *separate* cases all the same - a reader that
    /// dropped a package-less id would still fail on `name_of_id`, so only the
    /// registered-but-node-less case exercises the node lookup at all.
    ///
    /// The edge is added below the root, proving the failure survives the
    /// frontier rather than being noticed only at the entry point.
    #[test]
    fn reachable_reports_an_edge_it_cannot_resolve(
        (shape, root) in graph(),
        missing in package_name(),
        package_is_registered in any::<bool>(),
    ) {
        prop_assume!(!shape.names.contains(&missing));
        let mut metadata = render(&shape);
        if package_is_registered {
            metadata
                .packages
                .push(package(&missing, &PackageId::of(&missing)));
        }
        let root_id = PackageId::of(&root).to_string();
        for node in &mut metadata.resolve.nodes {
            if node.id == root_id {
                node.deps.push(dependency(&PackageId::of(&missing)));
            }
        }
        prop_assert!(
            metadata
                .reachable(&root)
                .is_err_and(|error| format!("{error:#}").contains(&missing)),
            "an edge to the unresolved identity {missing} must be reported",
        );
    }
}

/// A version-split package must be traversed through every id it owns.
///
/// `reachable` walks package *ids* and only then maps them to names and
/// de-duplicates. That order is load-bearing, and the generated families above
/// cannot show it: they give one id per name, so a closure that stopped after
/// the first id bearing a name, or de-duplicated ids before traversing, would
/// still agree with the oracle on every case they produce.
///
/// One name with two versions is the ordinary Cargo document that exposes the
/// difference. Both ids are distinct, both are reachable, and their
/// dependency branches are disjoint, so collapsing to one id drops a reachable
/// package entirely while the shared name still appears once.
///
/// The `edges` helper resolves a name to its *first* id, so it cannot describe
/// a version-split graph either; the assertion below therefore names the id
/// rather than asking for edges by name.
#[test]
fn reachable_merges_two_ids_that_share_a_name() {
    // `old` and `new` are two versions of one package. Each has its own
    // dependency, so the branches are distinguishable in the result.
    let split = "split";
    let old = PackageId::of("old");
    let new = PackageId::of("new");
    let graph = Metadata {
        packages: vec![
            package("root", &PackageId::of("root")),
            package(split, &old),
            package(split, &new),
            package("left", &PackageId::of("left")),
            package("right", &PackageId::of("right")),
        ],
        resolve: Resolve {
            nodes: vec![
                node(&PackageId::of("root"), &[old.clone(), new.clone()]),
                node(&old, &[PackageId::of("left")]),
                node(&new, &[PackageId::of("right")]),
                node(&PackageId::of("left"), &[]),
                node(&PackageId::of("right"), &[]),
            ],
        },
    };

    let reachable = graph
        .reachable("root")
        .expect("a version-split graph is a document Cargo emits");

    // Both branches survived, which is what proves traversal continued past
    // the first id rather than stopping at the name it had already seen.
    assert!(
        reachable.contains(&"left"),
        "left is reachable: {reachable:?}"
    );
    assert!(
        reachable.contains(&"right"),
        "right is reachable through the second id: {reachable:?}"
    );
    // The shared name appears, and appears once.
    assert_eq!(
        reachable.iter().filter(|name| **name == split).count(),
        1,
        "the shared name is reported exactly once: {reachable:?}",
    );
}
