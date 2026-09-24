//! Tests for policy evaluation and the D7 sanity findings.

use super::*;
use crate::policy::config::{PolicyConfig, PolicyException};
use proptest::prelude::*;
use rstest::rstest;

fn config_with(exceptions: Vec<PolicyException>) -> PolicyConfig {
    PolicyConfig {
        mode: crate::policy::PolicyMode::Warn,
        exceptions,
    }
}

fn exception(kind: ExceptionKind, name: &str) -> PolicyException {
    PolicyException {
        kind,
        name: name.to_owned(),
        reason: "test".to_owned(),
        command_path: None,
    }
}

#[rstest]
fn empty_config_produces_empty_report_with_vocabulary() {
    let report = evaluate(&PolicyConfig::default(), &PolicyInputs::default());

    assert_eq!(report.mode, crate::policy::PolicyMode::Off);
    assert!(report.results.is_empty());
    assert!(report.exceptions.is_empty());
    assert_eq!(
        report.vocabulary.verbs,
        CANONICAL_VERBS
            .iter()
            .map(|verb| (*verb).to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        report.vocabulary.flags,
        CANONICAL_FLAGS
            .iter()
            .map(|flag| (*flag).to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(report.summary.total, 0);
}

#[rstest]
fn explicit_off_mode_suppresses_all_findings() {
    let config = PolicyConfig {
        mode: crate::policy::PolicyMode::Off,
        exceptions: vec![
            exception(ExceptionKind::Flag, ""),
            exception(ExceptionKind::Verb, "get"),
            exception(ExceptionKind::Flag, "--format"),
            exception(ExceptionKind::Flag, "--format"),
        ],
    };
    let report = evaluate(&config, &PolicyInputs::default());

    assert_eq!(report.mode, crate::policy::PolicyMode::Off);
    assert!(
        report.results.is_empty(),
        "off mode must suppress malformed, redundant, and duplicate findings"
    );
    assert_eq!(report.summary.total, 0);
    assert_eq!(report.exceptions.len(), 4);
}

#[rstest]
fn report_attaches_configured_exceptions() {
    let report = evaluate(
        &config_with(vec![exception(ExceptionKind::Verb, "get")]),
        &PolicyInputs::default(),
    );

    assert_eq!(report.exceptions.len(), 1);
    let attached = report.exceptions.first().expect("one exception");
    assert_eq!(attached.name, "get");
}

#[rstest]
#[case(ExceptionKind::Flag, "--json")]
#[case(ExceptionKind::Flag, "json")]
#[case(ExceptionKind::Verb, "get")]
fn redundant_canonical_exception_is_warned(#[case] kind: ExceptionKind, #[case] name: &str) {
    let report = evaluate(
        &config_with(vec![exception(kind, name)]),
        &PolicyInputs::default(),
    );

    assert_eq!(report.results.len(), 1);
    let result = report.results.first().expect("one finding");
    assert_eq!(result.code, "redundant_exception");
    assert_eq!(result.severity, PolicySeverity::Warn);
}

#[rstest]
#[case(ExceptionKind::Flag, "--format")]
#[case(ExceptionKind::Verb, "info")]
fn non_canonical_exception_is_not_warned(#[case] kind: ExceptionKind, #[case] name: &str) {
    let report = evaluate(
        &config_with(vec![exception(kind, name)]),
        &PolicyInputs::default(),
    );

    assert!(report.results.is_empty());
}

#[rstest]
#[case(ExceptionKind::Flag, "")]
#[case(ExceptionKind::Flag, "--")]
#[case(ExceptionKind::Flag, "--bad flag")]
#[case(ExceptionKind::Flag, "bad flag")]
#[case(ExceptionKind::Verb, "")]
#[case(ExceptionKind::Verb, "bad verb")]
#[case(ExceptionKind::Verb, "-get")]
fn malformed_exception_is_denied(#[case] kind: ExceptionKind, #[case] name: &str) {
    let report = evaluate(
        &config_with(vec![exception(kind, name)]),
        &PolicyInputs::default(),
    );

    assert_eq!(report.results.len(), 1);
    let result = report.results.first().expect("one finding");
    assert_eq!(result.code, "malformed_exception");
    assert_eq!(result.severity, PolicySeverity::Deny);
}

#[rstest]
fn duplicate_exception_is_warned_once_per_repeat() {
    let report = evaluate(
        &config_with(vec![
            exception(ExceptionKind::Flag, "--format"),
            exception(ExceptionKind::Flag, "--format"),
        ]),
        &PolicyInputs::default(),
    );

    let duplicates = report
        .results
        .iter()
        .filter(|result| result.code == "duplicate_exception")
        .collect::<Vec<_>>();
    assert_eq!(duplicates.len(), 1);
    let duplicate = duplicates.first().expect("one duplicate finding");
    assert_eq!(duplicate.severity, PolicySeverity::Warn);
}

#[rstest]
fn scoped_exceptions_do_not_duplicate_across_scopes() {
    let mut first = exception(ExceptionKind::Flag, "--format");
    first.command_path = Some("remote".to_owned());
    let mut second = exception(ExceptionKind::Flag, "--format");
    second.command_path = Some("local".to_owned());

    let report = evaluate(&config_with(vec![first, second]), &PolicyInputs::default());

    assert!(
        report
            .results
            .iter()
            .all(|result| result.code != "duplicate_exception"),
        "exceptions with distinct scopes are not duplicates"
    );
}

#[rstest]
fn summary_counts_severities_across_findings() {
    let report = evaluate(
        &config_with(vec![
            exception(ExceptionKind::Flag, ""),
            exception(ExceptionKind::Verb, "get"),
            exception(ExceptionKind::Flag, "--format"),
        ]),
        &PolicyInputs::default(),
    );

    assert_eq!(report.summary.deny, 1);
    assert_eq!(report.summary.warn, 1);
    assert_eq!(report.summary.total, 2);
}

#[rstest]
fn findings_are_ordered_malformed_then_redundant_then_duplicate() {
    let report = evaluate(
        &config_with(vec![
            exception(ExceptionKind::Flag, ""),
            exception(ExceptionKind::Verb, "get"),
            exception(ExceptionKind::Flag, "--format"),
            exception(ExceptionKind::Flag, "--format"),
        ]),
        &PolicyInputs::default(),
    );

    let codes = report
        .results
        .iter()
        .map(|result| result.code.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        vec![
            "malformed_exception",
            "redundant_exception",
            "duplicate_exception"
        ]
    );
}

#[derive(Debug, Clone, Copy)]
enum ExceptionForm {
    EmptyFlag,
    SpacedVerb,
    CanonicalFlag,
    CanonicalVerb,
    CustomFlag,
    CustomVerb,
    ScopedCanonicalFlagA,
    ScopedCanonicalFlagB,
}

impl ExceptionForm {
    fn into_exception(self) -> PolicyException {
        match self {
            Self::EmptyFlag => exception(ExceptionKind::Flag, ""),
            Self::SpacedVerb => exception(ExceptionKind::Verb, "not a verb"),
            Self::CanonicalFlag => exception(ExceptionKind::Flag, "--json"),
            Self::CanonicalVerb => exception(ExceptionKind::Verb, "get"),
            Self::CustomFlag => exception(ExceptionKind::Flag, "--custom"),
            Self::CustomVerb => exception(ExceptionKind::Verb, "custom"),
            Self::ScopedCanonicalFlagA => scoped_exception("first"),
            Self::ScopedCanonicalFlagB => scoped_exception("second"),
        }
    }

    const fn is_malformed(self) -> bool {
        matches!(self, Self::EmptyFlag | Self::SpacedVerb)
    }

    const fn is_redundant(self) -> bool {
        matches!(
            self,
            Self::CanonicalFlag
                | Self::CanonicalVerb
                | Self::ScopedCanonicalFlagA
                | Self::ScopedCanonicalFlagB
        )
    }
}

fn scoped_exception(scope: &str) -> PolicyException {
    let mut exception = exception(ExceptionKind::Flag, "--json");
    exception.command_path = Some(scope.to_owned());
    exception
}

fn any_exception_form() -> impl Strategy<Value = ExceptionForm> {
    prop_oneof![
        Just(ExceptionForm::EmptyFlag),
        Just(ExceptionForm::SpacedVerb),
        Just(ExceptionForm::CanonicalFlag),
        Just(ExceptionForm::CanonicalVerb),
        Just(ExceptionForm::CustomFlag),
        Just(ExceptionForm::CustomVerb),
        Just(ExceptionForm::ScopedCanonicalFlagA),
        Just(ExceptionForm::ScopedCanonicalFlagB),
    ]
}

fn any_policy_mode() -> impl Strategy<Value = PolicyMode> {
    prop_oneof![
        Just(PolicyMode::Off),
        Just(PolicyMode::Warn),
        Just(PolicyMode::Deny),
    ]
}

fn duplicate_count(exceptions: &[PolicyException]) -> usize {
    exceptions
        .iter()
        .enumerate()
        .filter(|(index, exception)| {
            exceptions.iter().take(*index).any(|previous| {
                previous.kind == exception.kind
                    && previous.name == exception.name
                    && previous.command_path == exception.command_path
            })
        })
        .count()
}

proptest! {
    #[test]
    fn evaluation_preserves_policy_finding_invariants(
        mode in any_policy_mode(),
        forms in prop::collection::vec(any_exception_form(), 0..24),
    ) {
        let exceptions = forms
            .iter()
            .copied()
            .map(ExceptionForm::into_exception)
            .collect::<Vec<_>>();
        let report = evaluate(
            &PolicyConfig { mode, exceptions: exceptions.clone() },
            &PolicyInputs::default(),
        );

        prop_assert_eq!(&report.exceptions, &exceptions);
        if mode == PolicyMode::Off {
            prop_assert!(report.results.is_empty());
            prop_assert_eq!(report.summary.total, 0);
        } else {
            let malformed = forms.iter().filter(|form| form.is_malformed()).count();
            let redundant = forms.iter().filter(|form| form.is_redundant()).count();
            let duplicates = duplicate_count(&report.exceptions);
            let actual_codes = report
                .results
                .iter()
                .map(|result| result.code.as_str())
                .collect::<Vec<_>>();
            let expected_codes = std::iter::repeat_n("malformed_exception", malformed)
                .chain(std::iter::repeat_n("redundant_exception", redundant))
                .chain(std::iter::repeat_n("duplicate_exception", duplicates))
                .collect::<Vec<_>>();
            let actual_severities = report
                .results
                .iter()
                .map(|result| result.severity.clone())
                .collect::<Vec<_>>();
            let expected_severities = std::iter::repeat_n(PolicySeverity::Deny, malformed)
                .chain(std::iter::repeat_n(
                    PolicySeverity::Warn,
                    redundant + duplicates,
                ))
                .collect::<Vec<_>>();

            prop_assert_eq!(actual_codes, expected_codes);
            prop_assert_eq!(actual_severities, expected_severities);
        }
    }
}
