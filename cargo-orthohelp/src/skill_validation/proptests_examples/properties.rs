//! Properties and mutant controls for normalized-example binding.
//!
//! These live beside the generator and oracle they use: a child module sees its
//! parent's private items, so the data model stays in one file and the
//! assertions stay readable here.

use std::collections::BTreeSet;

use proptest::prelude::*;

use crate::policy::PolicyMode;

use super::super::input::{NormalizedExample, OptionUse};
use super::super::report::{ValidationStatus, codes};
use super::super::support::{command, example, input, manifest, policy, reference, surface};
use super::super::validate;
use super::{Spec, build_example, case_strategy, example_comparables, oracle, spec_strategy};

proptest! {
    /// The validator and the linear oracle agree on every generated example.
    #[test]
    fn example_findings_match_the_linear_oracle(case in case_strategy()) {
        let (predicted, coverage) = oracle(&case);
        let report = validate(&case.to_input(), &case.to_surface(), &policy(PolicyMode::Off))
            .expect("generated surfaces declare each path at most once");

        let comparable: Vec<(String, usize, String, Option<String>)> = predicted
            .into_iter()
            .map(|finding| {
                (
                    finding.manifest_id,
                    finding.source_ordinal,
                    finding.code.to_owned(),
                    finding.option,
                )
            })
            .collect();
        prop_assert_eq!(example_comparables(&report), comparable);
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
