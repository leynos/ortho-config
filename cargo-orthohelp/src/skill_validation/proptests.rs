//! Property tests for skill validation against an independent linear oracle.
//!
//! The oracle below re-derives the expected structural findings from the
//! generated case with its own loops, rather than calling the validator's
//! index or resolution code. A shared helper would let one defect satisfy both
//! sides of the comparison.
//!
//! The generator deliberately produces empty, singleton, nested, matching,
//! missing, and reordered cases: some paths are absent from the surface, some
//! references declare flags the command does not accept, and manifest IDs may
//! repeat.
//!
//! The deterministic controls live in the sibling `proptests_controls` module,
//! because they hold inputs fixed rather than generating them. One of them runs
//! [`oracle_matching_paths_by_length`] beside [`oracle`] on a single case, so
//! the path equality the property compares has a witness on that input rather
//! than only across different ones; another corrupts a real report and shows
//! the comparison rejects it, so it cannot pass vacuously.

use proptest::prelude::*;

use crate::policy::PolicyMode;

use super::input::{EffectiveCommandSurface, SkillValidationInput};
use super::support::{command, input, manifest, policy, reference, surface};
use super::{SkillValidationReport, validate};

/// Fixed command paths the generator may place on the surface.
const PATHS: &[&[&str]] = &[
    &["tool"],
    &["tool", "alpha"],
    &["tool", "beta"],
    &["tool", "alias"],
];

/// Fixed option names a command or a claim may use.
const FLAGS: &[&str] = &["json", "color", "bogus"];

/// Returns the path at `index`, falling back to the root command.
///
/// The fallback is unreachable for generated indices, but returning a value
/// keeps this helper total without relying on an `expect` outside a test.
fn segments(index: usize) -> &'static [&'static str] {
    match PATHS.get(index) {
        Some(path) => path,
        None => &["tool"],
    }
}

/// Returns the flag name for one bit position.
fn flag_name(bit: usize) -> &'static str {
    FLAGS.get(bit).copied().unwrap_or("json")
}

/// Expands an option bitmask into ascending flag names.
fn flag_names(mask: u8) -> Vec<&'static str> {
    (0..FLAGS.len())
        .filter(|bit| mask & (1 << bit) != 0)
        .map(flag_name)
        .collect()
}

/// One generated validation case.
#[derive(Debug, Clone)]
pub(super) struct Case {
    /// Declared paths, each with its option bitmask.
    pub(super) surface: Vec<(usize, u8)>,
    /// Manifests as an ID index plus references of path index and option mask.
    pub(super) manifests: Vec<(u8, Vec<(usize, u8)>)>,
}

impl Case {
    /// Builds the effective command surface this case describes.
    pub(super) fn to_surface(&self) -> EffectiveCommandSurface {
        let commands = self
            .surface
            .iter()
            .map(|(path_index, mask)| {
                let names = flag_names(*mask);
                command(segments(*path_index), &names)
            })
            .collect();
        surface(commands)
    }

    /// Builds the claim input this case describes.
    pub(super) fn to_input(&self) -> SkillValidationInput {
        let manifests = self
            .manifests
            .iter()
            .map(|(id_index, references)| {
                let id = format!("m{id_index}");
                let claims = references
                    .iter()
                    .map(|(path_index, mask)| {
                        let names = flag_names(*mask);
                        reference(segments(*path_index), &names)
                    })
                    .collect();
                manifest(&id, claims)
            })
            .collect();
        input(manifests, Vec::new())
    }
}

/// Generates varied surfaces, claims, and duplicate IDs.
fn case_strategy() -> impl Strategy<Value = Case> {
    let masks = prop::collection::vec(0..8u8, PATHS.len());
    let references = prop::collection::vec((0..PATHS.len(), 0..8u8), 0..=3);
    let manifests = prop::collection::vec((0..2u8, references), 0..=3);

    (masks, manifests).prop_map(|(surface_masks, claim_manifests)| Case {
        surface: surface_masks
            .into_iter()
            .enumerate()
            .filter(|(_, mask)| *mask != 0)
            .collect(),
        manifests: claim_manifests,
    })
}

/// Names the flags a reference uses that the surface does not declare.
fn unknown_flags(used: u8, declared: u8) -> Vec<String> {
    (0..FLAGS.len())
        .filter(|bit| used & (1 << bit) != 0 && declared & (1 << bit) == 0)
        .map(|bit| flag_name(bit).to_owned())
        .collect()
}

/// A finding reduced to the fields the oracle can predict.
pub(super) type Comparable = (String, usize, String, Option<String>);

/// Returns the surface option mask for one path, if declared.
fn surface_mask(case: &Case, path_index: usize) -> Option<u8> {
    case.surface
        .iter()
        .find(|(index, _)| *index == path_index)
        .map(|(_, mask)| *mask)
}

/// Returns the option mask of the first declared path with the same length.
///
/// This is the mutant lookup for [`oracle_matching_paths_by_length`]: it
/// degrades path equality to a segment count, which is exactly the defect a
/// comparison of stored path names would miss.
fn surface_mask_by_length(case: &Case, path_index: usize) -> Option<u8> {
    let wanted = segments(path_index).len();
    case.surface
        .iter()
        .find(|(index, _)| segments(*index).len() == wanted)
        .map(|(_, mask)| *mask)
}

/// Re-derives the expected findings, total references, and resolved count.
pub(super) fn oracle(case: &Case) -> (Vec<(usize, Comparable)>, usize, usize) {
    oracle_matching(case, surface_mask)
}

/// Re-derives the expectations a path-length comparison would produce.
///
/// Running this beside [`oracle`] on one fixed input is the control for
/// [`findings_match_the_linear_oracle`]: it shows the equality under test is
/// load-bearing on that input rather than only across different cases.
pub(super) fn oracle_matching_paths_by_length(
    case: &Case,
) -> (Vec<(usize, Comparable)>, usize, usize) {
    oracle_matching(case, surface_mask_by_length)
}

/// Re-derives the expected findings using `lookup` to resolve a declared path.
fn oracle_matching(
    case: &Case,
    lookup: fn(&Case, usize) -> Option<u8>,
) -> (Vec<(usize, Comparable)>, usize, usize) {
    let mut findings: Vec<(usize, Comparable)> = Vec::new();
    let mut total: usize = 0;
    let mut resolved: usize = 0;

    for (position, (id_index, references)) in case.manifests.iter().enumerate() {
        let id = format!("m{id_index}");
        let repeats = case
            .manifests
            .iter()
            .take(position)
            .any(|(earlier, _)| earlier == id_index);
        if repeats {
            findings.push((
                position,
                (
                    id.clone(),
                    position.saturating_add(1),
                    "skill.duplicate_manifest_id".to_owned(),
                    None,
                ),
            ));
        }
        for (reference_index, (path_index, mask)) in references.iter().enumerate() {
            total = total.saturating_add(1);
            let ordinal = reference_index.saturating_add(1);
            let Some(declared) = lookup(case, *path_index) else {
                findings.push((
                    position,
                    (id.clone(), ordinal, "skill.unknown_path".to_owned(), None),
                ));
                continue;
            };
            for flag in unknown_flags(*mask, declared) {
                findings.push((
                    position,
                    (
                        id.clone(),
                        ordinal,
                        "skill.unknown_flag".to_owned(),
                        Some(flag),
                    ),
                ));
            }
            if mask & !declared == 0 {
                resolved = resolved.saturating_add(1);
            }
        }
    }

    findings.sort_by(|(left_order, left), (right_order, right)| {
        left_order
            .cmp(right_order)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });
    (findings, total, resolved)
}

/// Reduces a report's findings to the comparable sequence.
pub(super) fn comparable(report: &SkillValidationReport) -> Vec<Comparable> {
    report
        .findings
        .iter()
        .map(|finding| {
            (
                finding.manifest_id.clone(),
                finding.source_ordinal,
                finding.code.clone(),
                finding.option.clone(),
            )
        })
        .collect()
}

proptest! {
    /// The validator and the linear oracle agree on every generated case.
    #[test]
    fn findings_match_the_linear_oracle(case in case_strategy()) {
        let (expected_with_order, total, resolved) = oracle(&case);
        let expected: Vec<Comparable> =
            expected_with_order.into_iter().map(|(_, finding)| finding).collect();
        let report = validate(&case.to_input(), &case.to_surface(), &policy(PolicyMode::Off))
            .expect("generated surfaces declare each path at most once");

        prop_assert_eq!(comparable(&report), expected);
        prop_assert_eq!(report.coverage.command_references, total);
        prop_assert_eq!(report.coverage.resolved_command_references, resolved);
    }

    /// Surface declaration order is not an input to the verdict.
    #[test]
    fn surface_order_does_not_change_the_report(case in case_strategy()) {
        let mut reversed_case = case.clone();
        reversed_case.surface.reverse();

        let forward = validate(&case.to_input(), &case.to_surface(), &policy(PolicyMode::Deny))
            .expect("generated surfaces are unambiguous");
        let reversed = validate(
            &reversed_case.to_input(),
            &reversed_case.to_surface(),
            &policy(PolicyMode::Deny),
        )
        .expect("generated surfaces are unambiguous");

        prop_assert_eq!(comparable(&forward), comparable(&reversed));
        prop_assert_eq!(forward.coverage, reversed.coverage);
    }
}
