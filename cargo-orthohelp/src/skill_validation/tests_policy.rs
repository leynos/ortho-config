//! Contract tests for the vocabulary and global-option policy.
//!
//! The policy layer is advisory: `off` silences it, `warn` and `deny` set the
//! finding severity, and configured exceptions suppress a specific flag. None
//! of that reaches structural findings, and a finding never echoes a supplied
//! argument value.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::policy_check::ResolvedAgentPolicy;
use super::report::{FindingCategory, codes};
use super::support::{
    codes_of, command, command_with_verb, example, flag_exception, input, manifest, only,
    option_use, policy, reference, surface,
};
use super::validate;
use crate::policy::{ExceptionKind, PolicyConfig, PolicyException, PolicyMode, PolicySeverity};

#[test]
fn policy_off_silences_vocabulary_but_not_structural_findings() {
    let commands = surface(vec![command(&["tool"], &["colour-policy"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![
                reference(&["tool"], &["colour-policy"]),
                reference(&["tool"], &["bogus"]),
            ],
        )],
        Vec::new(),
    );

    let report =
        validate(&claims, &commands, &policy(PolicyMode::Off)).expect("the surface is unambiguous");

    assert_eq!(codes_of(&report), vec![codes::UNKNOWN_FLAG]);
    assert!(!report.has_policy_findings());
}

#[rstest]
#[case(PolicyMode::Warn, PolicySeverity::Warn)]
#[case(PolicyMode::Deny, PolicySeverity::Deny)]
fn policy_mode_sets_the_finding_severity(
    #[case] mode: PolicyMode,
    #[case] severity: PolicySeverity,
) {
    let commands = surface(vec![command(&["tool"], &["colour-policy"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["colour-policy"])],
        )],
        Vec::new(),
    );

    let report = validate(&claims, &commands, &policy(mode)).expect("the surface is unambiguous");

    let finding = only(&report);
    assert_eq!(finding.category, FindingCategory::Policy);
    assert_eq!(finding.code, codes::POLICY_GLOBAL_OPTION_NEAR_MISS);
    assert_eq!(finding.severity, severity);
    assert!(
        finding.message.contains("--color"),
        "the advice must name the canonical option: {}",
        finding.message
    );
}

#[rstest]
#[case(&["tool"], None, Vec::new())]
#[case(
    &["tool", "render"],
    Some("tool build"),
    vec![codes::POLICY_GLOBAL_OPTION_NEAR_MISS]
)]
fn a_flag_exception_suppresses_advice_only_within_its_scope(
    #[case] command_path: &[&str],
    #[case] scope: Option<&str>,
    #[case] expected: Vec<&str>,
) {
    let commands = surface(vec![command(command_path, &["colour-policy"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(command_path, &["colour-policy"])],
        )],
        Vec::new(),
    );
    let excepting = ResolvedAgentPolicy {
        mode: PolicyMode::Deny,
        exceptions: vec![flag_exception("colour-policy", scope)],
    };

    let report = validate(&claims, &commands, &excepting).expect("the surface is unambiguous");

    assert_eq!(codes_of(&report), expected);
}

#[test]
fn an_exception_never_authorises_a_structural_unknown_flag() {
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["bogus"])],
        )],
        Vec::new(),
    );
    let exempt = ResolvedAgentPolicy {
        mode: PolicyMode::Deny,
        exceptions: vec![flag_exception("bogus", None)],
    };

    let report = validate(&claims, &commands, &exempt).expect("the surface is unambiguous");

    let finding = only(&report);
    assert_eq!(finding.code, codes::UNKNOWN_FLAG);
    assert_eq!(finding.category, FindingCategory::Structural);
}

#[test]
fn off_policy_verbs_and_flags_name_their_replacement() {
    let commands = surface(vec![command_with_verb(&["tool"], &["format"], "info")]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["format"])],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool"],
            vec![option_use("format", Some("json"), 1)],
        )],
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert_eq!(
        codes_of(&report),
        vec![
            codes::POLICY_NONCANONICAL_FLAG,
            codes::POLICY_NONCANONICAL_VERB
        ]
    );
    let verb = report
        .findings
        .iter()
        .find(|finding| finding.code == codes::POLICY_NONCANONICAL_VERB)
        .expect("the verb finding was asserted present");
    assert!(verb.message.contains("'get'"), "{}", verb.message);
}

#[test]
fn glossary_values_are_checked_without_echoing_them() {
    let commands = surface(vec![command(&["tool"], &["color"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["color"])],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool"],
            vec![option_use("color", Some("sekrit-value"), 1)],
        )],
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    let value_finding = report
        .findings
        .iter()
        .find(|finding| finding.code == codes::POLICY_GLOBAL_OPTION_VALUE)
        .expect("an unaccepted value must be reported");
    assert!(
        !value_finding.message.contains("sekrit-value"),
        "findings must never echo a supplied argument value: {}",
        value_finding.message
    );
    assert!(value_finding.message.contains("--color auto|always|never"));
}

#[test]
fn an_accepted_glossary_value_is_silent() {
    let commands = surface(vec![command(&["tool"], &["color"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["color"])],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool"],
            vec![option_use("color", Some("always"), 1)],
        )],
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert!(
        report.findings.is_empty(),
        "an accepted value and a bare declaration are both silent: {:?}",
        codes_of(&report)
    );
}

#[test]
fn resolved_policy_from_config_keeps_mode_and_exceptions() {
    let config = PolicyConfig {
        mode: PolicyMode::Warn,
        exceptions: vec![PolicyException {
            kind: ExceptionKind::Flag,
            name: "legacy".to_owned(),
            reason: "documented".to_owned(),
            command_path: Some("tool".to_owned()),
        }],
    };

    let resolved = ResolvedAgentPolicy::from(config);

    assert!(resolved.is_active());
    assert_eq!(resolved.severity(), PolicySeverity::Warn);
    assert_eq!(resolved.exceptions.len(), 1);
}
