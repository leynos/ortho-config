//! Contract tests for policy exceptions.
//!
//! An exception suppresses one named flag or verb, and only within its declared
//! command scope. Every case runs the same claim against the same surface and
//! changes only the exception, so a suppression that leaks to a neighbouring
//! command or to a different name shows up as an extra finding. An exception
//! never excuses a structural finding.

use pretty_assertions::assert_eq;
use rstest::rstest;

use super::input::EffectiveCommandSurface;
use super::policy_check::ResolvedAgentPolicy;
use super::report::{FindingCategory, codes};
use super::support::{
    codes_of, command, command_with_globals, command_with_verb, flag_exception, input, manifest,
    only, reference, surface, verb_exception,
};
use super::validate;
use crate::policy::{PolicyException, PolicyMode};

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
