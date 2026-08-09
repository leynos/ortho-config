//! Policy evaluation for configuration sanity.
//!
//! Evaluates a resolved [`PolicyConfig`] against [`PolicyInputs`] and returns
//! a [`PolicyReport`]. In 7.1.1 the inputs are empty and the evaluator emits
//! configuration-sanity findings only (Decision D7): malformed exceptions
//! (severity `deny`), redundant exceptions (severity `warn`), and duplicate
//! exceptions (severity `warn`). Roadmap 7.1.2 adds vocabulary lint rules on
//! the bridge IR passed through [`PolicyInputs`].

use crate::policy::config::{ExceptionKind, PolicyConfig, PolicyException, PolicyInputs};
use crate::policy::vocabulary::{
    CANONICAL_FLAGS, CANONICAL_VERBS, is_canonical_flag, is_canonical_verb,
};
use crate::policy::{PolicyReport, PolicyResult, PolicySeverity, Vocabulary};

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
    let mut results = Vec::new();
    for exception in &config.exceptions {
        if is_malformed(exception) {
            results.push(malformed_result(exception));
        }
    }
    for exception in &config.exceptions {
        if is_redundant(exception) {
            results.push(redundant_result(exception));
        }
    }
    for (index, exception) in config.exceptions.iter().enumerate() {
        if config
            .exceptions
            .iter()
            .take(index)
            .any(|previous| same_scope(previous, exception))
        {
            results.push(duplicate_result(exception));
        }
    }
    PolicyReport::with_details(
        config.mode,
        results,
        config.exceptions.clone(),
        canonical_vocabulary(),
    )
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

fn malformed_result(exception: &PolicyException) -> PolicyResult {
    PolicyResult {
        rule_id: "agent-native.config.malformed-exception".to_owned(),
        code: "malformed_exception".to_owned(),
        severity: PolicySeverity::Deny,
        message: format!(
            "malformed {} exception '{}': a {} name must be a single non-empty token",
            exception.kind, exception.name, exception.kind
        ),
        location: None,
    }
}

fn redundant_result(exception: &PolicyException) -> PolicyResult {
    PolicyResult {
        rule_id: "agent-native.config.redundant-exception".to_owned(),
        code: "redundant_exception".to_owned(),
        severity: PolicySeverity::Warn,
        message: format!(
            "exception for {} '{}' is redundant: the item is already canonical",
            exception.kind, exception.name
        ),
        location: None,
    }
}

fn duplicate_result(exception: &PolicyException) -> PolicyResult {
    PolicyResult {
        rule_id: "agent-native.config.duplicate-exception".to_owned(),
        code: "duplicate_exception".to_owned(),
        severity: PolicySeverity::Warn,
        message: format!(
            "duplicate {} exception '{}' with the same scope",
            exception.kind, exception.name
        ),
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
