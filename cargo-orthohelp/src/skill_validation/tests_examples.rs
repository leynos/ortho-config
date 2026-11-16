//! Contract tests for normalized-example binding.
//!
//! An example must bind to exactly one manifest reference, follow that
//! reference's path, and use only options the reference declares *and* the
//! referenced command accepts. The last two tests pin the rule that the
//! effective surface is per command: a root-only option never leaks into a
//! child, and a child that declares a global option does resolve it.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::report::{ValidationStatus, codes};
use super::support::{
    codes_of, command, command_with_globals, example, input, manifest, option_use, policy,
    reference, surface,
};
use super::validate;
use crate::policy::PolicyMode;

#[test]
fn example_for_a_missing_manifest_is_reported() {
    let claims = input(
        Vec::new(),
        vec![example("absent", 1, &["tool"], Vec::new())],
    );

    let report = validate(&claims, &surface(Vec::new()), &policy(PolicyMode::Off))
        .expect("the surface is unambiguous");

    assert_eq!(codes_of(&report), vec![codes::EXAMPLE_UNKNOWN_MANIFEST]);
}

#[test]
fn example_for_an_ambiguous_manifest_is_reported() {
    let claims = input(
        vec![manifest("dup", Vec::new()), manifest("dup", Vec::new())],
        vec![example("dup", 1, &["tool"], Vec::new())],
    );

    let report = validate(&claims, &surface(Vec::new()), &policy(PolicyMode::Off))
        .expect("the surface is unambiguous");

    assert_eq!(
        codes_of(&report),
        vec![
            codes::EXAMPLE_AMBIGUOUS_MANIFEST,
            codes::DUPLICATE_MANIFEST_ID
        ],
        "the example is anchored to the first manifest with that ID, \
         so it sorts before the repeat that created the ambiguity"
    );
}

#[test]
fn example_reference_and_path_must_match_the_manifest() {
    let commands = surface(vec![
        command(&["tool", "alpha"], &[]),
        command(&["tool", "beta"], &[]),
    ]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool", "alpha"], &[])],
        )],
        vec![
            example("operation-skill", 9, &["tool", "alpha"], Vec::new()),
            example("operation-skill", 1, &["tool", "beta"], Vec::new()),
        ],
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(
        codes_of(&report),
        vec![
            codes::EXAMPLE_PATH_MISMATCH,
            codes::EXAMPLE_UNBOUND_REFERENCE
        ],
        "findings follow the examples' source ordinals"
    );
}

#[rstest]
#[case::undeclared_and_unaccepted(&["json"], vec![codes::EXAMPLE_UNDECLARED_OPTION])]
#[case::declared_but_unaccepted(
    &["json", "yaml"],
    vec![codes::EXAMPLE_UNKNOWN_OPTION, codes::UNKNOWN_FLAG]
)]
fn example_options_are_reported_once_at_their_first_failure(
    #[case] declared: &[&str],
    #[case] expected: Vec<&str>,
) {
    // One option name may fail both the ownership and the membership check. The
    // plan allows at most one finding per example option, and the reference is
    // the claim the example elaborates, so ownership wins and reality is not
    // re-reported for the same token.
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], declared)],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool"],
            vec![option_use("yaml", None, 1)],
        )],
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(codes_of(&report), expected);
}

#[test]
fn a_root_only_option_is_not_available_on_a_child_command() {
    let commands = surface(vec![
        command(&["tool"], &["root-only"]),
        command(&["tool", "build"], &["json"]),
    ]);
    let claims = input(
        vec![manifest(
            "build-workflow",
            vec![reference(&["tool", "build"], &["root-only"])],
        )],
        vec![example(
            "build-workflow",
            1,
            &["tool", "build"],
            vec![option_use("root-only", None, 1)],
        )],
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(
        codes_of(&report),
        vec![codes::EXAMPLE_UNKNOWN_OPTION, codes::UNKNOWN_FLAG],
        "the effective surface must not union the root command's inputs"
    );
}

#[test]
fn an_example_whose_path_the_surface_lacks_is_not_counted_bound() {
    // The manifest declares the reference, so the example binds to it, but the
    // surface does not declare that path. No option can then be accepted and
    // the later option checks never run, so the run must not claim the example
    // was validated. The missing path is already reported once against the
    // manifest, so the example adds no finding of its own.
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool", "paint"], &["json", "yaml"])],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool", "paint"],
            vec![option_use("json", None, 1), option_use("yaml", None, 2)],
        )],
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(codes_of(&report), vec![codes::UNKNOWN_PATH]);
    assert_eq!(report.coverage.examples_checked, 1);
    assert_eq!(
        report.coverage.examples_bound, 0,
        "an example whose options were never checked is not bound"
    );
    assert_eq!(
        report.status,
        ValidationStatus::Partial,
        "a partially checked example must not report completeness"
    );
}

#[test]
fn an_optionless_example_whose_path_the_surface_lacks_is_not_counted_bound() {
    // No option needs the surface, so the previous rule had nothing to stop at
    // and counted the example as bound. Its path was still never checked, so
    // the run must not report completeness.
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool", "paint"], &["json"])],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool", "paint"],
            Vec::new(),
        )],
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(codes_of(&report), vec![codes::UNKNOWN_PATH]);
    assert_eq!(report.coverage.examples_checked, 1);
    assert_eq!(
        report.coverage.examples_bound, 0,
        "an optionless example on a missing path was never checked"
    );
    assert_eq!(report.status, ValidationStatus::Partial);
}

#[test]
fn an_undeclared_option_does_not_bind_an_example_to_a_missing_path() {
    // The undeclared option is still reported, because ownership needs only
    // the reference, but the example does not become bound merely because its
    // single option was classified without consulting the surface.
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool", "paint"], &["json"])],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool", "paint"],
            vec![option_use("other", None, 1)],
        )],
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(
        codes_of(&report),
        vec![codes::EXAMPLE_UNDECLARED_OPTION, codes::UNKNOWN_PATH]
    );
    assert_eq!(report.coverage.examples_checked, 1);
    assert_eq!(
        report.coverage.examples_bound, 0,
        "a missing path leaves the example unbound"
    );
    assert_eq!(report.status, ValidationStatus::Partial);
}

#[test]
fn a_declared_child_global_option_resolves() {
    // The child declares `color` itself and the consumer classifies it global;
    // the example then resolves and reaches its option checks.
    let commands = surface(vec![command_with_globals(
        &["tool", "build"],
        &["json", "color"],
        &["color"],
    )]);
    let claims = input(
        vec![manifest(
            "build-workflow",
            vec![reference(&["tool", "build"], &["json", "color"])],
        )],
        vec![example(
            "build-workflow",
            1,
            &["tool", "build"],
            vec![option_use("color", Some("always"), 1)],
        )],
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert_eq!(report.findings, Vec::new());
    assert_eq!(report.coverage.examples_checked, 1);
    assert_eq!(report.coverage.examples_bound, 1);
}
