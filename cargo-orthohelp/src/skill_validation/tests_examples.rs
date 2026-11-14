//! Contract tests for normalized-example binding.
//!
//! An example must bind to exactly one manifest reference, follow that
//! reference's path, and use only options the reference declares *and* the
//! referenced command accepts. The last two tests pin the rule that the
//! effective surface is per command: a root-only option never leaks into a
//! child, and a child that declares a global option does resolve it.

use pretty_assertions::assert_eq;

use super::report::codes;
use super::support::{
    codes_of, command, example, input, manifest, option_use, policy, reference, surface,
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

#[test]
fn example_options_must_be_declared_and_real() {
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["json"])],
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

    assert_eq!(
        codes_of(&report),
        vec![
            codes::EXAMPLE_UNDECLARED_OPTION,
            codes::EXAMPLE_UNKNOWN_OPTION
        ]
    );
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
fn a_declared_child_global_option_resolves() {
    let commands = surface(vec![
        command(&["tool"], &["json"]),
        command(&["tool", "build"], &["json"]),
    ]);
    let claims = input(
        vec![manifest(
            "build-workflow",
            vec![reference(&["tool", "build"], &["json"])],
        )],
        vec![example(
            "build-workflow",
            1,
            &["tool", "build"],
            vec![option_use("json", None, 1)],
        )],
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert_eq!(report.findings, Vec::new());
    assert_eq!(report.coverage.examples_checked, 1);
}
