//! Contract tests for supplied-claim validation.
//!
//! These cover the surface-independent behaviour: empty input, reference
//! resolution, exact name comparison, deterministic ordering, repeatability,
//! and the typed rejection of an ambiguous effective surface. Example binding
//! lives in `tests_examples`; policy and vocabulary behaviour lives in
//! `tests_policy`.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::assertions::{assert_clean, assert_partial, assert_structural};
use super::input::{
    CommandContract, CommandSurfaceError, EffectiveCommandSurface, EffectiveOption,
    ExampleExtraction,
};
use super::report::{Coverage, codes};
use super::support::{
    codes_of, command, command_with_verb, example, input, manifest, only, option_use, path, policy,
    reference, surface, validate_tool_references,
};
use super::validate;
use crate::policy::PolicyMode;

#[test]
fn empty_input_is_complete_and_empty() {
    let report = validate(
        &input(Vec::new(), Vec::new()),
        &surface(Vec::new()),
        &policy(PolicyMode::Deny),
    )
    .expect("an empty surface is unambiguous");

    assert_eq!(report.coverage, Coverage::default());
    assert_clean(&report);
}

#[test]
fn resolved_nested_reference_produces_no_findings() {
    let commands = surface(vec![
        command(&["tool"], &["json"]),
        command_with_verb(&["tool", "records", "get"], &["json"], "get"),
    ]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool", "records", "get"], &["json"])],
        )],
        Vec::new(),
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert_eq!(report.findings, Vec::new());
    assert_eq!(
        report.coverage,
        Coverage {
            command_references: 1,
            resolved_command_references: 1,
            examples_checked: 0,
            examples_unchecked: 0,
        }
    );
}

/// What an unresolvable reference is anchored to in its finding.
#[derive(Clone, Copy)]
enum Anchor {
    /// The whole command path, which the surface does not declare.
    Path(&'static [&'static str]),
    /// One flag name, which the resolved command does not accept.
    Option(&'static str),
}

#[rstest]
#[case(
    &["tool", "missing"],
    &["json", "bogus"],
    codes::UNKNOWN_PATH,
    Anchor::Path(&["tool", "missing"])
)]
#[case(
    &["tool"],
    &["json", "yaml", "yaml"],
    codes::UNKNOWN_FLAG,
    Anchor::Option("yaml")
)]
fn an_unresolvable_reference_yields_one_anchored_finding(
    #[case] reference_path: &[&str],
    #[case] flags: &[&str],
    #[case] code: &str,
    #[case] anchor: Anchor,
) {
    let report = validate_tool_references(vec![reference(reference_path, flags)], PolicyMode::Off);

    let finding = only(&report);
    assert_structural(finding, code);
    match anchor {
        Anchor::Path(segments) => assert_eq!(finding.command_path, path(segments)),
        Anchor::Option(name) => assert_eq!(finding.option.as_deref(), Some(name)),
    }
    assert_eq!(report.coverage.resolved_command_references, 0);
}

#[test]
fn duplicate_manifest_id_is_reported_on_the_repeat() {
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![
            manifest("dup", vec![reference(&["tool"], &["json"])]),
            manifest("dup", vec![reference(&["tool"], &["json"])]),
        ],
        Vec::new(),
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    let finding = only(&report);
    assert_eq!(finding.code, codes::DUPLICATE_MANIFEST_ID);
    assert_eq!(
        (finding.manifest_id.as_str(), finding.source_ordinal),
        ("dup", 2),
        "the second manifest is the repeat"
    );
    assert_eq!(
        report.coverage.resolved_command_references, 2,
        "a duplicate ID does not invalidate the references themselves"
    );
}

#[test]
fn option_names_are_compared_exactly() {
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["JSON", "--json"])],
        )],
        Vec::new(),
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert_eq!(
        codes_of(&report),
        vec![codes::UNKNOWN_FLAG, codes::UNKNOWN_FLAG],
        "case and a leading `--` are structurally significant"
    );
}

#[test]
fn unsupported_examples_are_unchecked_and_make_the_report_partial() {
    let mut skipped = example(
        "operation-skill",
        1,
        &["nowhere"],
        vec![option_use("x", None, 1)],
    );
    skipped.extraction = ExampleExtraction::Unsupported;
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["json"])],
        )],
        vec![skipped],
    );
    let commands = surface(vec![command(&["tool"], &["json"])]);

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(report.findings, Vec::new());
    assert_eq!(
        (
            report.coverage.examples_checked,
            report.coverage.examples_unchecked
        ),
        (0, 1)
    );
    assert_partial(&report);
}

#[test]
fn findings_are_ordered_by_manifest_then_ordinal_then_code() {
    let commands = surface(vec![command(&["tool"], &[])]);
    let claims = input(
        vec![
            manifest("second", vec![reference(&["missing"], &[])]),
            manifest(
                "first",
                vec![reference(&["tool"], &["a"]), reference(&["tool"], &["b"])],
            ),
        ],
        Vec::new(),
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    let ordinals: Vec<(String, usize)> = report
        .findings
        .iter()
        .map(|finding| (finding.manifest_id.clone(), finding.source_ordinal))
        .collect();
    assert_eq!(
        ordinals,
        vec![
            ("second".to_owned(), 1),
            ("first".to_owned(), 1),
            ("first".to_owned(), 2),
        ]
    );
}

#[test]
fn repeating_a_run_is_byte_stable() {
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["json", "a", "b"])],
        )],
        Vec::new(),
    );

    let first = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");
    let second = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert_eq!(first, second);
}

#[test]
fn surface_order_does_not_change_findings() {
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool", "build"], &["json"])],
        )],
        Vec::new(),
    );
    let forwards = surface(vec![
        command(&["tool"], &[]),
        command(&["tool", "build"], &["json"]),
    ]);
    let backwards = surface(vec![
        command(&["tool", "build"], &["json"]),
        command(&["tool"], &[]),
    ]);

    let first =
        validate(&claims, &forwards, &policy(PolicyMode::Off)).expect("the surface is unambiguous");
    let second = validate(&claims, &backwards, &policy(PolicyMode::Off))
        .expect("the surface is unambiguous");

    assert_eq!(first, second);
    assert_eq!(first.findings, Vec::new());
}

#[rstest]
#[case(
    surface(vec![command(&[], &[])]),
    CommandSurfaceError::EmptyPath { index: 0 }
)]
#[case(
    surface(vec![
        command(&["tool"], &[]),
        command(&["tool"], &["json"]),
    ]),
    CommandSurfaceError::DuplicatePath { path: "tool".to_owned() }
)]
#[case(
    surface(vec![command(&["tool"], &[""])]),
    CommandSurfaceError::EmptyOption { path: "tool".to_owned() }
)]
#[case(
    surface(vec![CommandContract {
        path: vec!["tool".to_owned()],
        options: vec![
            EffectiveOption {
                long: "json".to_owned(),
                value_type: Some("bool".to_owned()),
            },
            EffectiveOption {
                long: "json".to_owned(),
                value_type: Some("string".to_owned()),
            },
        ],
        canonical_verb: None,
    }]),
    CommandSurfaceError::ConflictingOption {
        path: "tool".to_owned(),
        option: "json".to_owned(),
    }
)]
fn ambiguous_surfaces_are_rejected_before_any_claim(
    #[case] commands: EffectiveCommandSurface,
    #[case] expected: CommandSurfaceError,
) {
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["json"])],
        )],
        Vec::new(),
    );

    let error = validate(&claims, &commands, &policy(PolicyMode::Off))
        .expect_err("an ambiguous surface must be rejected");

    assert_eq!(error, expected);
}
