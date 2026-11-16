//! Deterministic mutation controls for the reference property tests.
//!
//! The properties in [`super::proptests`] compare the validator with an
//! independent oracle, and that comparison only means something if changing
//! the rule changes the prediction. Each control here holds one input fixed and
//! mutates either the claim or the oracle, so it has a witness on that exact
//! input rather than only across two different surfaces.

use super::proptests::{Case, Comparable, comparable, oracle, oracle_matching_paths_by_length};
use super::report::codes;
use super::support::{command, input, manifest, policy, reference, surface};
use super::validate;
use crate::policy::PolicyMode;

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

#[test]
fn a_length_based_path_match_disagrees_on_the_same_input() {
    // `findings_match_the_linear_oracle` compares the validator with an oracle
    // that resolves a path by identity. Degrading that lookup to a segment
    // count, and running both oracles on one input, shows the equality the
    // property tests is load-bearing here: the mutant matches `tool beta`
    // against `tool alpha` and stays silent, while the validator reports the
    // path it cannot resolve.
    let case = Case {
        surface: vec![(1, 0b011)],
        manifests: vec![(0, vec![(2, 0b001)])],
    };
    let report = validate(
        &case.to_input(),
        &case.to_surface(),
        &policy(PolicyMode::Off),
    )
    .expect("the fixed surface is unambiguous");
    let observed = comparable(&report);
    let (honest, _, _) = oracle(&case);
    let (mutant, _, _) = oracle_matching_paths_by_length(&case);

    assert_eq!(
        observed,
        vec![("m0".to_owned(), 1, codes::UNKNOWN_PATH.to_owned(), None)],
        "the reference names a path the surface does not declare"
    );
    assert_eq!(
        observed,
        findings(honest),
        "the honest oracle must agree with the validator on this input"
    );
    assert_ne!(
        observed,
        findings(mutant),
        "matching paths by length must change the prediction for the same input"
    );
}

/// Drops the ordering key from an oracle's predictions.
fn findings(predictions: Vec<(usize, Comparable)>) -> Vec<Comparable> {
    predictions
        .into_iter()
        .map(|(_, finding)| finding)
        .collect()
}
