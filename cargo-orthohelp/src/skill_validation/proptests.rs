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
//! repeat. The tests at the end of the module mutate a claim and check that the
//! validator's verdict moves with it: dropping a reference, looking a flag up
//! on a sibling command, and swapping two commands' option sets each change the
//! result. A further control corrupts a real report and shows the comparison
//! above rejects it, so that comparison cannot pass vacuously.

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
struct Case {
    /// Declared paths, each with its option bitmask.
    surface: Vec<(usize, u8)>,
    /// Manifests as an ID index plus references of path index and option mask.
    manifests: Vec<(u8, Vec<(usize, u8)>)>,
}

impl Case {
    /// Builds the effective command surface this case describes.
    fn to_surface(&self) -> EffectiveCommandSurface {
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
    fn to_input(&self) -> SkillValidationInput {
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
type Comparable = (String, usize, String, Option<String>);

/// Returns the surface option mask for one path, if declared.
fn surface_mask(case: &Case, path_index: usize) -> Option<u8> {
    case.surface
        .iter()
        .find(|(index, _)| *index == path_index)
        .map(|(_, mask)| *mask)
}

/// Re-derives the expected findings, total references, and resolved count.
fn oracle(case: &Case) -> (Vec<(usize, Comparable)>, usize, usize) {
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
            let Some(declared) = surface_mask(case, *path_index) else {
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
fn comparable(report: &SkillValidationReport) -> Vec<Comparable> {
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

#[test]
fn the_oracle_comparison_rejects_a_corrupted_report() {
    // `findings_match_the_linear_oracle` compares two sequences; if either side
    // were constant, every case would pass. This control starts from a genuine
    // agreement, then drops one finding and shows the comparison notices.
    let case = Case {
        surface: vec![(0, 0b001)],
        manifests: vec![(0, vec![(0, 0b111)])],
    };
    let report = validate(
        &case.to_input(),
        &case.to_surface(),
        &policy(PolicyMode::Off),
    )
    .expect("the fixed surface is unambiguous");
    let (expected_with_order, _, _) = oracle(&case);
    let expected: Vec<Comparable> = expected_with_order
        .into_iter()
        .map(|(_, finding)| finding)
        .collect();

    assert_eq!(
        comparable(&report),
        expected,
        "the control must start agreed"
    );
    let mut corrupted = comparable(&report);
    corrupted.pop();
    assert_ne!(
        corrupted, expected,
        "a dropped finding must break the comparison"
    );
}

#[test]
fn omitting_a_declared_reference_is_detected() {
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let full = input(
        vec![manifest(
            "m0",
            vec![
                reference(&["tool"], &["json"]),
                reference(&["tool"], &["json"]),
            ],
        )],
        Vec::new(),
    );
    let mut trimmed = full.clone();
    if let Some(claims) = trimmed.manifests.first_mut() {
        claims.commands.pop();
    }

    let complete =
        validate(&full, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");
    let shortened = validate(&trimmed, &commands, &policy(PolicyMode::Off))
        .expect("the surface is unambiguous");

    assert_eq!(complete.coverage.command_references, 2);
    assert_eq!(shortened.coverage.command_references, 1);
}

#[test]
fn a_flag_accepted_by_a_sibling_command_is_still_unknown() {
    let commands = surface(vec![
        command(&["tool", "alpha"], &["json"]),
        command(&["tool", "beta"], &["color"]),
    ]);
    let claims = input(
        vec![manifest(
            "m0",
            vec![
                reference(&["tool", "beta"], &["json"]),
                reference(&["tool", "alpha"], &["json"]),
            ],
        )],
        Vec::new(),
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(report.findings.len(), 1);
    let finding = report.findings.first().expect("one finding was asserted");
    assert_eq!(finding.code, super::report::codes::UNKNOWN_FLAG);
    assert_eq!(finding.source_ordinal, 1);
}

#[test]
fn swapping_two_commands_option_sets_changes_the_verdict() {
    let claims = input(
        vec![manifest(
            "m0",
            vec![reference(&["tool", "alpha"], &["json"])],
        )],
        Vec::new(),
    );
    let accepted = surface(vec![command(&["tool", "alpha"], &["json"])]);
    let swapped = surface(vec![
        command(&["tool", "alpha"], &["color"]),
        command(&["tool", "beta"], &["json"]),
    ]);

    let without =
        validate(&claims, &accepted, &policy(PolicyMode::Off)).expect("the surface is unambiguous");
    let with =
        validate(&claims, &swapped, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(without.findings, Vec::new());
    assert_eq!(with.findings.len(), 1);
}
