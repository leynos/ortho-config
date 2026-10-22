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

/// Render `name` in Cargo's package-id spelling.
fn id(name: &str) -> String {
    format!("{ID_PREFIX}{name}")
}

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

/// A generated graph: its package names, its edges, and the root to close over.
///
/// Names come from a set, so they are unique and sorted - two packages cannot
/// share a name in one resolve, and generating a duplicate would ask a
/// question about a document Cargo never emits. The root is drawn from the
/// same names, so a generated case is never an accidental missing-root control.
fn graph() -> impl Strategy<Value = (Vec<String>, Vec<(usize, usize)>, String)> {
    btree_set(package_name(), 1..8).prop_flat_map(|generated| {
        let names: Vec<String> = generated.into_iter().collect();
        let count = names.len();
        let roots = names.clone();
        (
            Just(names),
            vec(edge(count), 0..12),
            prop::sample::select(roots),
        )
    })
}

/// Group generated edges by source, keeping one entry per distinct target set.
///
/// Out-of-range indices are skipped rather than indexed: `edge` never produces
/// them, and [`clippy::indexing_slicing`] is denied workspace-wide anyway.
fn adjacency<'a>(
    names: &'a [String],
    edges: &[(usize, usize)],
) -> BTreeMap<&'a str, BTreeSet<&'a str>> {
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (source_index, target_index) in edges {
        let (Some(source), Some(target)) = (names.get(*source_index), names.get(*target_index))
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

/// Render a graph into the `cargo metadata` shapes [`Metadata`] parses.
fn render(names: &[String], edges: &[(usize, usize)]) -> Metadata {
    let adjacency = adjacency(names, edges);
    let packages = names
        .iter()
        .map(|name| MetadataPackage {
            id: id(name),
            name: name.clone(),
            version: String::from("0.0.0"),
            source: None,
        })
        .collect();
    let nodes = names
        .iter()
        .map(|name| ResolveNode {
            id: id(name),
            deps: adjacency
                .get(name.as_str())
                .into_iter()
                .flatten()
                .map(|target| ResolveDependency {
                    pkg: id(target),
                    dep_kinds: Vec::new(),
                })
                .collect(),
        })
        .collect();
    Metadata {
        packages,
        resolve: Resolve { nodes },
    }
}

/// The textbook least fixed point over `edges`, as a set of names.
///
/// This is the oracle: start from the root alone and repeatedly add the
/// successors of everything already in the answer. No visited set and no
/// frontier - only the definition of reachability - so it cannot share the
/// implementation's bookkeeping defect.
fn reference_closure(names: &[String], edges: &[(usize, usize)], root: &str) -> BTreeSet<String> {
    let mut reached = BTreeSet::from([root.to_owned()]);
    loop {
        let mut grown = reached.clone();
        for (source_index, target_index) in edges {
            let (Some(source), Some(target)) = (names.get(*source_index), names.get(*target_index))
            else {
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
        (names, edges, root) in graph(),
    ) {
        let metadata = render(&names, &edges);
        let actual = metadata
            .reachable(&root)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let expected = reference_closure(&names, &edges, &root);
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
        (names, edges, root) in graph(),
        missing in package_name(),
    ) {
        prop_assume!(!names.contains(&missing));
        let metadata = render(&names, &edges);
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
        (names, edges, root) in graph(),
        missing in package_name(),
        package_is_registered in any::<bool>(),
    ) {
        prop_assume!(!names.contains(&missing));
        let mut metadata = render(&names, &edges);
        if package_is_registered {
            metadata.packages.push(MetadataPackage {
                id: id(&missing),
                name: missing.clone(),
                version: String::from("0.0.0"),
                source: None,
            });
        }
        let root_id = id(&root);
        for node in &mut metadata.resolve.nodes {
            if node.id == root_id {
                node.deps.push(ResolveDependency {
                    pkg: id(&missing),
                    dep_kinds: Vec::new(),
                });
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
