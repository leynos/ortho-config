//! Contract tests for the vocabulary and global-option policy.
//!
//! The policy layer is advisory: `off` silences it, `warn` and `deny` set the
//! finding severity, and configured exceptions suppress a specific flag. None
//! of that reaches structural findings, and a finding never echoes a supplied
//! argument value.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::input::EffectiveCommandSurface;
use super::policy_check::ResolvedAgentPolicy;
use super::report::{FindingCategory, codes};
use super::support::{
    codes_of, command, command_with_globals, command_with_verb, example, flag_exception, input,
    manifest, only, option_use, policy, reference, surface, verb_exception,
};
use super::validate;
use crate::policy::{ExceptionKind, PolicyConfig, PolicyException, PolicyMode, PolicySeverity};

#[test]
fn policy_off_silences_vocabulary_but_not_structural_findings() {
    let commands = surface(vec![command_with_globals(
        &["tool"],
        &["colour-policy"],
        &["colour-policy"],
    )]);
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
    let commands = surface(vec![command_with_globals(
        &["tool"],
        &["colour-policy"],
        &["colour-policy"],
    )]);
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

/// The command arrangement an exception case resolves against.
#[derive(Clone, Copy)]
enum Declared {
    /// A command accepting one off-policy flag.
    Flag,
    /// A command carrying a canonical flag and one off-policy verb.
    Verb,
}

impl Declared {
    /// Builds the surface this arrangement declares at `command_path`.
    fn surface(self, command_path: &[&str]) -> EffectiveCommandSurface {
        match self {
            Self::Flag => surface(vec![command_with_globals(
                command_path,
                &["colour-policy"],
                &["colour-policy"],
            )]),
            Self::Verb => surface(vec![command_with_verb(command_path, &["json"], "info")]),
        }
    }

    /// Returns the flags the claim declares.
    const fn flags(self) -> &'static [&'static str] {
        match self {
            Self::Flag => &["colour-policy"],
            Self::Verb => &["json"],
        }
    }

    /// Builds an exception of this arrangement's kind.
    fn exception(self, name: &str, scope: Option<&str>) -> PolicyException {
        match self {
            Self::Flag => flag_exception(name, scope),
            Self::Verb => verb_exception(name, scope),
        }
    }
}

/// One policy-exception case: what is declared, and what is exempted.
#[derive(Clone, Copy)]
struct ExceptionCase {
    /// Claim and surface arrangement under test.
    declared: Declared,
    /// Path both the surface and the claim use.
    command_path: &'static [&'static str],
    /// Name the exception grants.
    exception_name: &'static str,
    /// Optional command scope on the exception.
    scope: Option<&'static str>,
}

#[rstest]
#[case(
    ExceptionCase {
        declared: Declared::Flag,
        command_path: &["tool"],
        exception_name: "colour-policy",
        scope: None,
    },
    Vec::new()
)]
#[case(
    ExceptionCase {
        declared: Declared::Flag,
        command_path: &["tool", "render"],
        exception_name: "colour-policy",
        scope: Some("tool build"),
    },
    vec![codes::POLICY_GLOBAL_OPTION_NEAR_MISS]
)]
#[case(
    ExceptionCase {
        declared: Declared::Verb,
        command_path: &["tool"],
        exception_name: "info",
        scope: Some("tool"),
    },
    Vec::new()
)]
#[case(
    ExceptionCase {
        declared: Declared::Verb,
        command_path: &["tool"],
        exception_name: "info",
        scope: Some("tool records"),
    },
    vec![codes::POLICY_NONCANONICAL_VERB]
)]
#[case(
    ExceptionCase {
        declared: Declared::Verb,
        command_path: &["tool"],
        exception_name: "list",
        scope: Some("tool"),
    },
    vec![codes::POLICY_NONCANONICAL_VERB]
)]
fn an_exception_suppresses_advice_only_when_name_and_scope_match(
    #[case] case: ExceptionCase,
    #[case] expected: Vec<&str>,
) {
    let commands = case.declared.surface(case.command_path);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(case.command_path, case.declared.flags())],
        )],
        Vec::new(),
    );
    let excepting = ResolvedAgentPolicy {
        mode: PolicyMode::Deny,
        exceptions: vec![case.declared.exception(case.exception_name, case.scope)],
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
    let commands = surface(vec![command_with_globals(
        &["tool"],
        &["color"],
        &["color"],
    )]);
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
    let commands = surface(vec![command_with_globals(
        &["tool"],
        &["color"],
        &["color"],
    )]);
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
fn a_domain_option_is_not_checked_against_the_global_glossary() {
    // The surface does not classify this command's `color` as the global
    // colour-policy option, so a domain value must not be judged by the
    // glossary's accepted values. Without the classification the validator
    // would fail a perfectly good domain command.
    let commands = surface(vec![command(&["tool", "paint"], &["color"])]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool", "paint"], &["color"])],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool", "paint"],
            vec![option_use("color", Some("blue"), 1)],
        )],
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert!(
        report.findings.is_empty(),
        "a domain option must not receive global-option advice: {:?}",
        codes_of(&report)
    );
}

#[test]
fn policy_advice_requires_an_owned_example_option() {
    // The example's reference does not declare `color`, so the use never
    // resolved. Reporting a value finding as well would judge a claim the
    // example has already contradicted.
    let commands = surface(vec![command_with_globals(
        &["tool"],
        &["json", "color"],
        &["color"],
    )]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool"], &["json"])],
        )],
        vec![example(
            "operation-skill",
            1,
            &["tool"],
            vec![option_use("color", Some("sometimes"), 1)],
        )],
    );

    let report = validate(&claims, &commands, &policy(PolicyMode::Deny))
        .expect("the surface is unambiguous");

    assert_eq!(codes_of(&report), vec![codes::EXAMPLE_UNDECLARED_OPTION]);
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
