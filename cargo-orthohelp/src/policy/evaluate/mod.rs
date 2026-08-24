//! Policy evaluation for configuration sanity.
//!
//! Evaluates a resolved [`PolicyConfig`] against [`PolicyInputs`] and returns
//! a [`PolicyReport`]. In 7.1.1 the inputs are empty and the evaluator emits
//! configuration-sanity findings only (Decision D7): malformed exceptions
//! (severity `deny`), redundant exceptions (severity `warn`), and duplicate
//! exceptions (severity `warn`). An explicit `mode = "off"` configuration
//! suppresses evaluation entirely — the report carries zero findings while
//! still advertising the configured exceptions and the canonical vocabulary.
//! Roadmap 7.1.2 adds vocabulary lint rules on the bridge IR passed through
//! [`PolicyInputs`].

use crate::policy::config::{ExceptionKind, PolicyConfig, PolicyException, PolicyInputs};
use crate::policy::vocabulary::{
    CANONICAL_FLAGS, CANONICAL_VERBS, is_canonical_flag, is_canonical_verb,
};
use crate::policy::{PolicyMode, PolicyReport, PolicyResult, PolicySeverity, Vocabulary};

/// Evaluates a policy configuration and produces a report.
///
/// The report's `vocabulary` block is populated from the canonical default
/// constants, and its `exceptions` list reproduces every configured exception
/// so policy output is self-describing.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::policy::config::{PolicyConfig, PolicyInputs};
/// use cargo_orthohelp::policy::evaluate::evaluate;
/// use cargo_orthohelp::policy::PolicyMode;
///
/// let report = evaluate(&PolicyConfig::default(), &PolicyInputs::default());
/// assert_eq!(report.mode, PolicyMode::Off);
/// assert!(report.results.is_empty());
/// ```
#[must_use]
pub fn evaluate(config: &PolicyConfig, _inputs: &PolicyInputs) -> PolicyReport {
    let results = if config.mode == PolicyMode::Off {
        Vec::new()
    } else {
        let mut results = Vec::new();
        results.extend(malformed_results(&config.exceptions));
        results.extend(redundant_results(&config.exceptions));
        results.extend(duplicate_results(&config.exceptions));
        results
    };
    PolicyReport::with_details(
        config.mode,
        results,
        config.exceptions.clone(),
        canonical_vocabulary(),
    )
}

/// Collects a `malformed_exception` finding for every malformed exception.
fn malformed_results(exceptions: &[PolicyException]) -> Vec<PolicyResult> {
    exceptions
        .iter()
        .filter(|exception| is_malformed(exception))
        .map(|exception| {
            exception_result(
                "agent-native.config.malformed-exception",
                "malformed_exception",
                PolicySeverity::Deny,
                format!(
                    "malformed {} exception '{}': a {} name must be a single non-empty token",
                    exception.kind, exception.name, exception.kind
                ),
            )
        })
        .collect()
}

/// Collects a `redundant_exception` finding for every canonical exception.
fn redundant_results(exceptions: &[PolicyException]) -> Vec<PolicyResult> {
    exceptions
        .iter()
        .filter(|exception| is_redundant(exception))
        .map(|exception| {
            exception_result(
                "agent-native.config.redundant-exception",
                "redundant_exception",
                PolicySeverity::Warn,
                format!(
                    "exception for {} '{}' is redundant: the item is already canonical",
                    exception.kind, exception.name
                ),
            )
        })
        .collect()
}

/// Collects a `duplicate_exception` finding for every exception repeated by a
/// preceding exception with the same scope.
fn duplicate_results(exceptions: &[PolicyException]) -> Vec<PolicyResult> {
    let mut results = Vec::new();
    for (index, exception) in exceptions.iter().enumerate() {
        if exceptions
            .iter()
            .take(index)
            .any(|previous| same_scope(previous, exception))
        {
            results.push(exception_result(
                "agent-native.config.duplicate-exception",
                "duplicate_exception",
                PolicySeverity::Warn,
                format!(
                    "duplicate {} exception '{}' with the same scope",
                    exception.kind, exception.name
                ),
            ));
        }
    }
    results
}

/// Returns whether an exception's name cannot match its kind's shape.
///
/// The D7 boundary is precise: a `flag` exception whose name, after optional
/// `--` prefix normalization, is empty or contains whitespace; a `verb`
/// exception whose name is empty, contains whitespace, or begins with `-`.
fn is_malformed(exception: &PolicyException) -> bool {
    match exception.kind {
        ExceptionKind::Flag => {
            let name = exception.name.strip_prefix("--").unwrap_or(&exception.name);
            name.is_empty() || name.chars().any(char::is_whitespace)
        }
        ExceptionKind::Verb => {
            exception.name.is_empty()
                || exception.name.chars().any(char::is_whitespace)
                || exception.name.starts_with('-')
        }
    }
}

/// Returns whether an exception names an already-canonical vocabulary item.
fn is_redundant(exception: &PolicyException) -> bool {
    match exception.kind {
        ExceptionKind::Verb => is_canonical_verb(&exception.name),
        ExceptionKind::Flag => is_canonical_flag(&exception.name),
    }
}

/// Returns whether two exceptions share kind, name, and command scope.
fn same_scope(left: &PolicyException, right: &PolicyException) -> bool {
    left.kind == right.kind && left.name == right.name && left.command_path == right.command_path
}

/// Constructs a [`PolicyResult`] from its primitive parts.
fn exception_result(
    rule_id: &str,
    code: &str,
    severity: PolicySeverity,
    message: String,
) -> PolicyResult {
    PolicyResult {
        rule_id: rule_id.to_owned(),
        code: code.to_owned(),
        severity,
        message,
        location: None,
    }
}

fn canonical_vocabulary() -> Vocabulary {
    Vocabulary {
        verbs: CANONICAL_VERBS
            .iter()
            .map(|verb| (*verb).to_owned())
            .collect(),
        flags: CANONICAL_FLAGS
            .iter()
            .map(|flag| (*flag).to_owned())
            .collect(),
    }
}

#[cfg(test)]
mod tests;
