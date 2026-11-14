//! Properties and mutant controls for normalized-example binding.
//!
//! These live beside the generator and oracle they use: a child module sees its
//! parent's private items, so the data model stays in one file and the
//! assertions stay readable here.

use std::collections::BTreeSet;

use proptest::prelude::*;

use crate::policy::PolicyMode;

use super::super::input::{NormalizedExample, OptionUse, SkillValidationInput};
use super::super::report::{ValidationStatus, codes};
use super::super::support::{
    codes_of, command, command_with_globals, example, input, manifest, option_use, policy,
    reference, surface,
};
use super::super::validate;
use super::oracle::{example_comparables, expected_comparables, oracle};
use super::{Case, Spec, build_example, case_strategy, spec_strategy};

proptest! {
    /// The validator and the linear oracle agree on every generated example.
    #[test]
    fn example_findings_match_the_linear_oracle(case in case_strategy()) {
        let (predicted, coverage) = oracle(&case);
        let report = validate(&case.to_input(), &case.to_surface(), &policy(PolicyMode::Off))
            .expect("generated surfaces declare each path at most once");

        prop_assert_eq!(example_comparables(&report), expected_comparables(&predicted));
        prop_assert_eq!(report.coverage.examples_checked, coverage.checked);
        prop_assert_eq!(report.coverage.examples_unchecked, coverage.unchecked);
        prop_assert_eq!(report.coverage.examples_bound, coverage.bound);

        let is_complete = coverage.unchecked == 0 && coverage.bound == coverage.checked;
        let expected_status = if is_complete { ValidationStatus::Complete } else { ValidationStatus::Partial };
        prop_assert_eq!(report.status, expected_status);
    }

    /// One offending example option produces one finding, not one per check.
    #[test]
    fn each_offending_example_option_is_reported_once(case in case_strategy()) {
        let report = validate(&case.to_input(), &case.to_surface(), &policy(PolicyMode::Off))
            .expect("generated surfaces declare each path at most once");
        let mut seen = BTreeSet::new();
        for finding in &report.findings {
            let is_option_finding = finding.code == codes::EXAMPLE_UNDECLARED_OPTION
                || finding.code == codes::EXAMPLE_UNKNOWN_OPTION;
            if !is_option_finding {
                continue;
            }
            let Some(option) = finding.option.clone() else {
                prop_assert!(false, "an option finding must name its option");
                continue;
            };
            prop_assert!(
                seen.insert((finding.manifest_id.clone(), finding.source_ordinal, option)),
                "an example option was reported more than once"
            );
        }
    }

    /// No finding echoes a supplied argument value.
    #[test]
    fn findings_never_echo_a_supplied_value(case in case_strategy()) {
        let values: Vec<&str> = case
            .examples
            .iter()
            .flat_map(|spec| spec.options.iter())
            .filter_map(|(_, value)| value.as_deref())
            .filter(|value| !value.is_empty())
            .collect();
        let report = validate(&case.to_input(), &case.to_surface(), &policy(PolicyMode::Deny))
            .expect("generated surfaces declare each path at most once");
        for finding in &report.findings {
            for value in &values {
                prop_assert!(!finding.message.contains(value));
                prop_assert!(finding.option.as_deref() != Some(*value));
            }
        }
    }

    /// A normalized example survives serialization with every field intact.
    #[test]
    fn a_normalized_example_survives_a_json_round_trip(spec in spec_strategy()) {
        let example = build_example(&spec, 1, "m0");
        let encoded = serde_json::to_string(&example);
        prop_assert!(encoded.is_ok(), "the record must serialize: {encoded:?}");
        let decoded = encoded
            .ok()
            .and_then(|json| serde_json::from_str::<NormalizedExample>(&json).ok());
        prop_assert_eq!(decoded.as_ref(), Some(&example));
    }

    /// The whole supplied input reaches the validator unchanged through JSON.
    #[test]
    fn an_input_survives_its_json_form_without_changing_the_report(case in case_strategy()) {
        let commands = case.to_surface();
        let direct = validate(&case.to_input(), &commands, &policy(PolicyMode::Deny));
        prop_assert!(direct.is_ok(), "generated surfaces are unambiguous");

        let decoded = serde_json::to_string(&case.to_input())
            .ok()
            .and_then(|json| serde_json::from_str::<SkillValidationInput>(&json).ok());
        let round_tripped = decoded
            .and_then(|input| validate(&input, &commands, &policy(PolicyMode::Deny)).ok());
        prop_assert_eq!(direct.ok(), round_tripped);
    }
}

#[test]
fn the_round_trip_comparison_notices_a_dropped_value() {
    // The property above compares a decoded record with the original. This
    // control drops every value from the record and shows that comparison
    // notices, so value presence cannot pass unchecked.
    let spec = Spec {
        manifest: 0,
        is_extracted: true,
        ordinal: 1,
        path: 0,
        options: vec![(0, Some("always".to_owned())), (1, None)],
        positionals: vec!["input.txt".to_owned()],
    };
    let example = build_example(&spec, 1, "m0");
    let mut mutated = example.clone();
    for option in &mut mutated.options {
        option.value = None;
    }
    let dropped = serde_json::to_string(&mutated)
        .ok()
        .and_then(|json| serde_json::from_str::<NormalizedExample>(&json).ok());

    assert_ne!(
        dropped.as_ref(),
        Some(&example),
        "a dropped value must survive as a visible difference"
    );
}

#[test]
fn unioning_the_root_options_into_a_child_hides_the_finding() {
    // A mutant that resolves a child's effective options as the union of its
    // parent's would accept an option the real CLI rejects. The real surface
    // is compared against that mutant so the per-command rule has a witness.
    let claims = input(
        vec![manifest(
            "m0",
            vec![reference(&["tool", "build"], &["root-only"])],
        )],
        vec![example(
            "m0",
            1,
            &["tool", "build"],
            vec![OptionUse {
                name: "root-only".to_owned(),
                value: None,
                source_ordinal: 1,
            }],
        )],
    );
    let real = surface(vec![
        command(&["tool"], &["root-only"]),
        command(&["tool", "build"], &["json"]),
    ]);
    let unioned = surface(vec![
        command(&["tool"], &["root-only"]),
        command(&["tool", "build"], &["json", "root-only"]),
    ]);

    let real_report =
        validate(&claims, &real, &policy(PolicyMode::Off)).expect("the surface is unambiguous");
    let unioned_report =
        validate(&claims, &unioned, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(
        example_comparables(&real_report),
        vec![(
            "m0".to_owned(),
            1,
            codes::EXAMPLE_UNKNOWN_OPTION.to_owned(),
            Some("root-only".to_owned())
        )],
        "the real child does not accept its root's option"
    );
    assert!(
        example_comparables(&unioned_report).is_empty(),
        "the union mutant must hide the finding, proving the rule is what emits it"
    );
}

#[test]
fn a_value_kept_across_the_input_boundary_still_decides_the_advice() {
    // The consumer hands the validator a JSON form of what it extracted. A
    // value that crosses that boundary decides whether glossary advice fires:
    // `always` is accepted and silent, while the same record with `sometimes`
    // is reported. A consumer that dropped the value would produce a different
    // report, so value preservation is observable at the boundary, not only in
    // a Serde comparison.
    let commands = surface(vec![command_with_globals(
        &["tool", "paint"],
        &["color"],
        &["color"],
    )]);
    let claims = input(
        vec![manifest(
            "m0",
            vec![reference(&["tool", "paint"], &["color"])],
        )],
        vec![example(
            "m0",
            1,
            &["tool", "paint"],
            vec![option_use("color", Some("always"), 1)],
        )],
    );

    let accepted = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");
    assert!(
        accepted.findings.is_empty(),
        "an accepted glossary value is silent: {:?}",
        codes_of(&accepted)
    );

    let json = serde_json::to_string(&claims).expect("the supplied input serializes");
    let mut decoded: SkillValidationInput =
        serde_json::from_str(&json).expect("the supplied input decodes");
    let Some(option) = decoded
        .examples
        .first_mut()
        .and_then(|record| record.options.first_mut())
    else {
        panic!("the control supplies one example option");
    };
    option.value = Some("sometimes".to_owned());

    let changed = validate(&decoded, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");
    assert_eq!(
        codes_of(&changed),
        vec![codes::POLICY_GLOBAL_OPTION_VALUE],
        "the value itself must decide the advice"
    );
}

#[test]
fn ignoring_command_acceptance_changes_the_comparison() {
    // A declared option the command also accepts produces no finding, so a
    // mutant that ignored the acceptance branch would still agree with the
    // oracle on that case. Narrowing the surface to a different option makes
    // the validator report `unknown_option`, which the oracle does not
    // predict: the acceptance branch is load-bearing in the comparison.
    let agreed = Case {
        surface: vec![(0, 0b001)],
        manifests: vec![(0, vec![(0, 0b001)])],
        examples: vec![Spec {
            manifest: 0,
            is_extracted: true,
            ordinal: 1,
            path: 0,
            options: vec![(0, None)],
            positionals: Vec::new(),
        }],
    };
    let (predicted, _) = oracle(&agreed);
    let report = validate(
        &agreed.to_input(),
        &agreed.to_surface(),
        &policy(PolicyMode::Off),
    )
    .expect("the fixed surface is unambiguous");
    assert_eq!(
        example_comparables(&report),
        expected_comparables(&predicted),
        "the control must start agreed"
    );

    let narrowed = Case {
        surface: vec![(0, 0b010)],
        ..agreed
    };
    let narrowed_report = validate(
        &narrowed.to_input(),
        &narrowed.to_surface(),
        &policy(PolicyMode::Off),
    )
    .expect("the fixed surface is unambiguous");
    assert_eq!(
        example_comparables(&narrowed_report),
        vec![(
            "m0".to_owned(),
            1,
            codes::EXAMPLE_UNKNOWN_OPTION.to_owned(),
            Some("json".to_owned())
        )],
        "a declared but unaccepted option is reported"
    );
    assert_ne!(
        example_comparables(&narrowed_report),
        expected_comparables(&predicted),
        "the comparison must reject what the oracle did not predict"
    );
}

#[test]
fn swapping_ownership_and_acceptance_changes_the_finding() {
    // Ownership asks what the reference declares; acceptance asks what the
    // command offers. They are different predicates, so the same option name
    // produces a different code depending on which one fails. A mutant that
    // swapped them would report the other code.
    let accepting = surface(vec![command(&["tool"], &["json"])]);

    let undeclared = input(
        vec![manifest("m0", vec![reference(&["tool"], &[])])],
        vec![example(
            "m0",
            1,
            &["tool"],
            vec![option_use("json", None, 1)],
        )],
    );
    let undeclared_report =
        validate(&undeclared, &accepting, &policy(PolicyMode::Off)).expect("the surface is clean");
    assert_eq!(
        codes_of(&undeclared_report),
        vec![codes::EXAMPLE_UNDECLARED_OPTION],
        "the reference declares nothing, so ownership fails first"
    );

    let unaccepted = input(
        vec![manifest("m0", vec![reference(&["tool"], &["json"])])],
        vec![example(
            "m0",
            1,
            &["tool"],
            vec![option_use("json", None, 1)],
        )],
    );
    let unaccepted_report =
        validate(&unaccepted, &accepting, &policy(PolicyMode::Off)).expect("the surface is clean");
    assert_eq!(
        codes_of(&unaccepted_report),
        Vec::<&str>::new(),
        "a declared and accepted option is silent"
    );

    let rejecting = surface(vec![command(&["tool"], &["color"])]);
    let rejected_report =
        validate(&unaccepted, &rejecting, &policy(PolicyMode::Off)).expect("the surface is clean");
    assert_eq!(
        codes_of(&rejected_report),
        vec![codes::EXAMPLE_UNKNOWN_OPTION, codes::UNKNOWN_FLAG],
        "declared but unaccepted is a different failure from undeclared, and \
         the reference's own flag claim is reported too"
    );
}

#[test]
fn duplicate_references_do_not_union_their_flags() {
    // Two references may point at the same path with different flags. An
    // example binds to one ordinal, so it sees only that reference's flags;
    // unioning them would accept an option the claim never declared.
    let commands = surface(vec![command(&["tool"], &["json", "color"])]);
    let claims = input(
        vec![manifest(
            "m0",
            vec![
                reference(&["tool"], &["json"]),
                reference(&["tool"], &["color"]),
            ],
        )],
        vec![example(
            "m0",
            1,
            &["tool"],
            vec![option_use("color", None, 1)],
        )],
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");
    assert_eq!(
        codes_of(&report),
        vec![codes::EXAMPLE_UNDECLARED_OPTION],
        "the example sees only the flags of the reference it binds to"
    );
}
