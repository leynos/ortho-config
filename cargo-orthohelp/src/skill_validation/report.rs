//! Result types for skill validation.
//!
//! Structural and policy findings share one ordered list but keep distinct
//! categories, so a consumer can honour `off` mode for vocabulary advice
//! without ever hiding a false command claim.

use serde::{Deserialize, Serialize};

use crate::policy::PolicySeverity;

pub mod codes {
    //! Stable identifier strings for validator findings.
    //!
    //! Codes are part of the consumer-facing contract: consumers match on these
    //! strings, so a code is added rather than redefined.

    /// A manifest command reference names a path the surface does not declare.
    pub const UNKNOWN_PATH: &str = "skill.unknown_path";
    /// A declared flag is absent from the resolved command's options.
    pub const UNKNOWN_FLAG: &str = "skill.unknown_flag";
    /// Two or more manifests share one ID.
    pub const DUPLICATE_MANIFEST_ID: &str = "skill.duplicate_manifest_id";
    /// An example names a manifest ID shared by more than one manifest.
    pub const EXAMPLE_AMBIGUOUS_MANIFEST: &str = "skill.example.ambiguous_manifest";
    /// An example names a manifest ID the input does not declare.
    pub const EXAMPLE_UNKNOWN_MANIFEST: &str = "skill.example.unknown_manifest";
    /// An example's reference ordinal does not select a declared reference.
    pub const EXAMPLE_UNBOUND_REFERENCE: &str = "skill.example.unbound_reference";
    /// An example's path differs from the path of its owning reference.
    pub const EXAMPLE_PATH_MISMATCH: &str = "skill.example.path_mismatch";
    /// An example uses an option its owning reference does not declare.
    pub const EXAMPLE_UNDECLARED_OPTION: &str = "skill.example.undeclared_option";
    /// An example uses an option the real command does not accept.
    pub const EXAMPLE_UNKNOWN_OPTION: &str = "skill.example.unknown_option";
    /// A resolved flag or option is an off-policy vocabulary spelling.
    pub const POLICY_NONCANONICAL_FLAG: &str = "skill.policy.noncanonical_flag";
    /// A resolved command declares an off-policy verb.
    pub const POLICY_NONCANONICAL_VERB: &str = "skill.policy.noncanonical_verb";
    /// A resolved option is a legacy spelling of a glossary concept.
    pub const POLICY_GLOBAL_OPTION_NEAR_MISS: &str = "skill.policy.global_option_near_miss";
    /// A glossary option is used without, or with an unaccepted, value.
    pub const POLICY_GLOBAL_OPTION_VALUE: &str = "skill.policy.global_option_value";
}

/// Whether a finding describes a broken claim or a policy preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCategory {
    /// The claim cannot be true against the supplied command surface.
    Structural,
    /// The claim resolves but offends the configured vocabulary policy.
    Policy,
}

/// One validation finding.
///
/// Findings quote the stable manifest ID and a one-based source ordinal. They
/// never quote a manifest filesystem path or an example argument value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationFinding {
    /// Structural or policy origin.
    pub category: FindingCategory,
    /// Stable finding code, for example [`codes::UNKNOWN_FLAG`].
    pub code: String,
    /// Severity under the configured policy mode.
    pub severity: PolicySeverity,
    /// Stable manifest identifier.
    pub manifest_id: String,
    /// One-based source ordinal of the claim that produced the finding.
    pub source_ordinal: usize,
    /// Exact command path the claim names.
    pub command_path: Vec<String>,
    /// Option or flag name, without the leading `--`, when relevant.
    pub option: Option<String>,
    /// Human-readable diagnostic.
    pub message: String,
}

/// Deterministic coverage counts for one validation call.
///
/// Counts are descriptive. They do not assert that a manifest's prose was
/// exhaustively understood.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    /// Command references supplied by all manifests.
    pub command_references: usize,
    /// References whose path and every declared flag resolved.
    pub resolved_command_references: usize,
    /// Examples the consumer extracted completely.
    pub examples_checked: usize,
    /// Examples the consumer marked unsupported.
    pub examples_unchecked: usize,
}

/// Whether the supplied claims were validated completely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    /// Every supplied example was checked.
    Complete,
    /// At least one example was unsupported and therefore unchecked.
    Partial,
}

/// The result of one validation call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillValidationReport {
    /// Findings ordered by manifest input order, source ordinal, then code.
    pub findings: Vec<ValidationFinding>,
    /// Descriptive coverage counts.
    pub coverage: Coverage,
    /// Completeness of example checking.
    pub status: ValidationStatus,
}

impl SkillValidationReport {
    /// Returns whether any structural finding is present.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_orthohelp::skill_validation::{
    ///     Coverage, SkillValidationReport, ValidationStatus,
    /// };
    ///
    /// let report = SkillValidationReport {
    ///     findings: Vec::new(),
    ///     coverage: Coverage::default(),
    ///     status: ValidationStatus::Complete,
    /// };
    /// assert!(!report.has_structural_findings());
    /// ```
    #[must_use]
    pub fn has_structural_findings(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.category == FindingCategory::Structural)
    }

    /// Returns whether any policy finding is present.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_orthohelp::skill_validation::{
    ///     Coverage, SkillValidationReport, ValidationStatus,
    /// };
    ///
    /// let report = SkillValidationReport {
    ///     findings: Vec::new(),
    ///     coverage: Coverage::default(),
    ///     status: ValidationStatus::Complete,
    /// };
    /// assert!(!report.has_policy_findings());
    /// ```
    #[must_use]
    pub fn has_policy_findings(&self) -> bool {
        self.findings
            .iter()
            .any(|finding| finding.category == FindingCategory::Policy)
    }

    /// Returns whether validation covered every supplied example.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cargo_orthohelp::skill_validation::{
    ///     Coverage, SkillValidationReport, ValidationStatus,
    /// };
    ///
    /// let report = SkillValidationReport {
    ///     findings: Vec::new(),
    ///     coverage: Coverage::default(),
    ///     status: ValidationStatus::Partial,
    /// };
    /// assert!(!report.is_complete());
    /// ```
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.status == ValidationStatus::Complete
    }
}
