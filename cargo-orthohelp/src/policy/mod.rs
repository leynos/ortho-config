//! Policy report schema emitted by `cargo-orthohelp`.
//!
//! The report is a narrow machine-readable contract for agent-native warnings
//! and hard failures. It is owned by this tool until a later design decision
//! extracts a reusable report model into a lower crate.

use serde::{Deserialize, Serialize};

/// Current policy-report schema version.
pub const ORTHO_POLICY_REPORT_SCHEMA_VERSION: &str = "1";

/// Machine-readable policy report for one `cargo-orthohelp` run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyReport {
    /// Policy-report schema version string.
    pub version: String,
    /// Tool that emitted the report.
    pub tool: String,
    /// Enforcement mode used by the run.
    pub mode: PolicyMode,
    /// Individual policy findings.
    pub results: Vec<PolicyResult>,
    /// Count summary grouped by severity.
    #[serde(default)]
    pub summary: PolicySummary,
}

impl PolicyReport {
    /// Creates an empty report for the supplied enforcement mode.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_orthohelp::policy::{PolicyMode, PolicyReport};
    ///
    /// let report = PolicyReport::empty(PolicyMode::Warn);
    /// assert_eq!(report.tool, "cargo-orthohelp");
    /// assert!(report.results.is_empty());
    /// ```
    #[must_use]
    pub fn empty(mode: PolicyMode) -> Self {
        Self {
            version: ORTHO_POLICY_REPORT_SCHEMA_VERSION.to_owned(),
            tool: "cargo-orthohelp".to_owned(),
            mode,
            results: Vec::new(),
            summary: PolicySummary::default(),
        }
    }

    /// Creates a report and derives the summary from the supplied results.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_orthohelp::policy::{PolicyMode, PolicyReport};
    ///
    /// let report = PolicyReport::with_results(PolicyMode::Warn, Vec::new());
    /// assert_eq!(report.summary.total, 0);
    /// ```
    #[must_use]
    pub fn with_results(mode: PolicyMode, results: Vec<PolicyResult>) -> Self {
        Self {
            summary: PolicySummary::from_results(&results),
            results,
            ..Self::empty(mode)
        }
    }
}

/// Enforcement mode selected for policy evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    /// Do not evaluate policy checks.
    Off,
    /// Emit findings without making them fatal.
    Warn,
    /// Treat deny-level findings as hard failures.
    Deny,
}

/// One policy finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyResult {
    /// Stable policy rule identifier.
    pub rule_id: String,
    /// Machine-readable finding code.
    pub code: String,
    /// Finding severity.
    pub severity: PolicySeverity,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Optional source location.
    #[serde(default)]
    pub location: Option<SourceLocation>,
}

/// Severity of one policy finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicySeverity {
    /// Suppressed or informational result.
    Off,
    /// Non-fatal policy warning.
    Warn,
    /// Hard policy failure.
    Deny,
}

/// Optional source location for a policy result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceLocation {
    /// Repository-relative or package-relative file path.
    pub file: String,
    /// Optional start and end range.
    #[serde(default)]
    pub range: Option<SourceRange>,
}

/// One-based source range.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRange {
    /// Start position.
    pub start: SourcePosition,
    /// End position.
    pub end: SourcePosition,
}

/// One-based source position.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourcePosition {
    /// One-based line number.
    pub line: u32,
    /// One-based column number.
    pub column: u32,
}

/// Count summary grouped by policy severity.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySummary {
    /// Number of off-level results.
    pub off: usize,
    /// Number of warning results.
    pub warn: usize,
    /// Number of deny results.
    pub deny: usize,
    /// Total number of results.
    pub total: usize,
}

impl PolicySummary {
    /// Builds a summary from policy results.
    #[must_use]
    pub fn from_results(results: &[PolicyResult]) -> Self {
        let mut summary = Self::default();
        for result in results {
            match result.severity {
                PolicySeverity::Off => summary.off += 1,
                PolicySeverity::Warn => summary.warn += 1,
                PolicySeverity::Deny => summary.deny += 1,
            }
        }
        summary.total = results.len();
        summary
    }
}

#[cfg(test)]
mod tests;
