//! The descendant walk must find every reachable process exactly once.
//!
//! [`super::descendants_from`] decides which process identifiers the teardown
//! kills, so a walk that missed a reachable process would leave a compiler
//! writing to the leased target, and one that recorded an unrelated process
//! would signal something outside the build. The generated forests vary depth,
//! branching, repeated child links, and unrelated trees; the expected set is
//! computed by an independent fixpoint over the same links rather than by
//! walking the way the implementation does.

use super::descendants_from;
use anyhow::{Result, ensure};
use proptest::prelude::*;
use std::collections::{BTreeSet, HashMap};

/// The root, and the smallest identifier a generated forest uses.
///
/// Zero is a child in no generated edge, which mirrors a real process table:
/// the root cannot be its own descendant, so every reachable identifier is a
/// strictly greater pid.
const ROOT: u8 = 0;

/// The number of process identifiers a generated forest may use.
const PROCESSES: u8 = 12;

/// Generate parent-to-child links over [`PROCESSES`] identifiers.
///
/// Children are drawn from `1..PROCESSES`, so an identifier can never appear
/// as its own child and the root is never a child. Links are drawn with
/// replacement, which produces the repeated links and the disconnected trees
/// the walk has to tolerate.
fn arb_forest() -> impl Strategy<Value = Vec<(u8, u8)>> {
    prop::collection::vec((0..PROCESSES, 1..PROCESSES), 0..usize::from(PROCESSES) * 3)
}

/// Build the adjacency map the walk consumes, without reusing its fold.
fn adjacency(links: &[(u8, u8)]) -> HashMap<u32, Vec<u32>> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for (parent, child) in links {
        children
            .entry(u32::from(*parent))
            .or_default()
            .push(u32::from(*child));
    }
    children
}

/// Every identifier reachable from [`ROOT`], as an independent fixpoint.
///
/// The set is grown from the root until an iteration adds nothing, which is a
/// different algorithm from the walk's depth-first frontier. Duplicate links
/// are irrelevant to a set, so the model ignores them by construction.
fn reachable(links: &[(u8, u8)]) -> BTreeSet<u32> {
    let mut found = BTreeSet::new();
    loop {
        let before = found.len();
        for (parent, child) in links {
            if u32::from(*parent) == u32::from(ROOT) || found.contains(&u32::from(*parent)) {
                found.insert(u32::from(*child));
            }
        }
        if found.len() == before {
            return found;
        }
    }
}

proptest! {
    /// Every reachable identifier appears once; nothing else appears at all.
    #[test]
    fn the_walk_matches_reachability(links in arb_forest()) {
        let walked = descendants_from(&adjacency(&links), u32::from(ROOT));
        let expected = reachable(&links);
        let unique = walked.iter().copied().collect::<BTreeSet<_>>();
        prop_assert_eq!(
            walked.len(),
            unique.len(),
            "the walk recorded a pid more than once: {:?}",
            walked,
        );
        prop_assert_eq!(
            &unique,
            &expected,
            "the walk recorded {:?} but the reachable set is {:?} (links: {:?})",
            unique,
            expected,
            links,
        );
        prop_assert!(
            !unique.contains(&u32::from(ROOT)),
            "the root is not its own descendant",
        );
    }
}

/// Every recorded pid is reached through the root or another recorded pid.
///
/// The property above fixes membership, but a walk that recorded a pid reached
/// through an unrelated tree would still be answering the wrong question. Each
/// case here has an unrelated tree and a chain into it, so a walk that started
/// from the wrong node would be visible.
#[test]
fn every_recorded_pid_has_a_recorded_or_root_parent() -> Result<()> {
    for links in [
        vec![(0, 1), (1, 2), (2, 3)],
        vec![(0, 1), (0, 2), (1, 3), (1, 4), (2, 4)],
        vec![(5, 6), (6, 7), (0, 5)],
    ] {
        let children = adjacency(&links);
        let walked = descendants_from(&children, u32::from(ROOT));
        for child in &walked {
            let has_reachable_parent = links.iter().any(|(parent, candidate)| {
                u32::from(*candidate) == *child
                    && (*parent == ROOT || walked.contains(&u32::from(*parent)))
            });
            ensure!(
                has_reachable_parent,
                "pid {child} was recorded without a reachable parent (links: {links:?})",
            );
        }
    }
    Ok(())
}
