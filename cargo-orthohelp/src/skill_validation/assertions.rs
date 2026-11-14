//! Assertion helpers shared by the skill-validation unit tests.
//!
//! Each helper states one domain expectation about a report, so a test names
//! the contract it checks instead of repeating the same block of raw
//! assertions around every call.

use pretty_assertions::assert_eq;

use super::report::{FindingCategory, SkillValidationReport, ValidationFinding, ValidationStatus};
use crate::policy::PolicySeverity;

/// Asserts one finding is structural and therefore denied in every mode.
pub(crate) fn assert_structural(finding: &ValidationFinding, code: &str) {
    assert_eq!(finding.code, code);
    assert_eq!(finding.category, FindingCategory::Structural);
    assert_eq!(
        finding.severity,
        PolicySeverity::Deny,
        "structural findings ignore the policy mode"
    );
}

/// Asserts a report found nothing and checked everything it was given.
pub(crate) fn assert_clean(report: &SkillValidationReport) {
    assert_eq!(report.findings, Vec::new());
    assert_eq!(report.status, ValidationStatus::Complete);
    assert!(report.is_complete());
}

/// Asserts a report is partial because some examples stayed unchecked.
pub(crate) fn assert_partial(report: &SkillValidationReport) {
    assert_eq!(report.status, ValidationStatus::Partial);
    assert!(!report.is_complete());
}
