//! Validation orchestration for supplied skill claims.
//!
//! Structural findings are produced for every broken claim regardless of the
//! policy mode. Policy advice is produced only for claims that resolved. The
//! final report is ordered by manifest input order, source ordinal, then
//! finding code, so repeated runs over the same input are byte-stable.

use std::collections::BTreeSet;

use ortho_config::SkillCommandRef;

use self::manifest_index::ManifestIndex;
use super::input::{
    CommandSurfaceError, EffectiveCommandSurface, ExampleExtraction, ManifestClaims,
    SkillValidationInput,
};
use super::policy_check::{PolicyAdvice, ResolvedAgentPolicy, flag_name_advice, verb_advice};
use super::report::{
    Coverage, FindingCategory, SkillValidationReport, ValidationFinding, ValidationStatus, codes,
};
use super::resolve::{CommandIndex, join_path};

mod examples;
mod manifest_index;

/// A finding anchored to one supplied claim.
#[derive(Clone, Copy)]
struct ClaimRef<'a> {
    /// Position of the owning manifest in the input list.
    order: usize,
    /// Stable manifest identifier.
    manifest_id: &'a str,
    /// One-based source ordinal of the offending claim.
    source_ordinal: usize,
    /// Exact command path the claim names.
    path: &'a [String],
}

/// A structural finding before it is anchored and ordered.
struct FindingData {
    /// Stable finding code.
    code: &'static str,
    /// Option or flag name when relevant.
    option: Option<String>,
    /// Human-readable diagnostic.
    message: String,
}

/// State for one validation call.
struct Validator<'a> {
    /// Exact command-surface lookup.
    index: CommandIndex,
    /// ID-keyed manifest view.
    manifests: ManifestIndex<'a>,
    /// Policy configuration.
    policy: &'a ResolvedAgentPolicy,
    /// Findings tagged with their owning manifest's input position.
    findings: Vec<(usize, ValidationFinding)>,
    /// Descriptive coverage counts.
    coverage: Coverage,
}

impl<'a> Validator<'a> {
    /// Runs every structural and policy check and returns the ordered report.
    fn run(mut self, input: &'a SkillValidationInput) -> SkillValidationReport {
        self.coverage.command_references = input.manifests.iter().map(|m| m.commands.len()).sum();
        for (position, manifest) in input.manifests.iter().enumerate() {
            self.check_duplicate_id(position, manifest);
            self.check_manifest(position, manifest);
        }
        for example in &input.examples {
            self.check_example(example);
        }
        self.coverage.examples_checked = count_extraction(input, ExampleExtraction::Checked);
        self.coverage.examples_unchecked = count_extraction(input, ExampleExtraction::Unsupported);
        let status = if self.coverage.examples_unchecked == 0 {
            ValidationStatus::Complete
        } else {
            ValidationStatus::Partial
        };
        self.findings
            .sort_by(|(left_order, left), (right_order, right)| {
                left_order
                    .cmp(right_order)
                    .then(left.source_ordinal.cmp(&right.source_ordinal))
                    .then(left.code.cmp(&right.code))
            });
        SkillValidationReport {
            findings: self
                .findings
                .into_iter()
                .map(|(_, finding)| finding)
                .collect(),
            coverage: self.coverage,
            status,
        }
    }

    /// Reports an ID repeated by an earlier manifest.
    fn check_duplicate_id(&mut self, position: usize, manifest: &'a ManifestClaims) {
        if !self.manifests.is_repeat(&manifest.id, position) {
            return;
        }
        self.push_structural(
            ClaimRef {
                order: position,
                manifest_id: &manifest.id,
                source_ordinal: position.saturating_add(1),
                path: &[],
            },
            FindingData {
                code: codes::DUPLICATE_MANIFEST_ID,
                option: None,
                message: format!(
                    "manifest ID '{}' is declared more than once; examples for it are ambiguous",
                    manifest.id
                ),
            },
        );
    }

    /// Checks every command reference in one manifest.
    fn check_manifest(&mut self, position: usize, manifest: &'a ManifestClaims) {
        for (index, reference) in manifest.commands.iter().enumerate() {
            let claim = ClaimRef {
                order: position,
                manifest_id: &manifest.id,
                source_ordinal: index.saturating_add(1),
                path: &reference.path,
            };
            self.check_reference(claim, reference);
        }
    }

    /// Checks one command reference, then its resolved policy.
    fn check_reference(&mut self, claim: ClaimRef<'a>, reference: &'a SkillCommandRef) {
        if self.index.command(&reference.path).is_none() {
            self.push_structural(
                claim,
                FindingData {
                    code: codes::UNKNOWN_PATH,
                    option: None,
                    message: format!(
                        "command path '{}' is not declared by the command surface",
                        join_path(&reference.path)
                    ),
                },
            );
            return;
        }
        let mut all_resolved = true;
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for flag in &reference.flags {
            if seen.insert(flag.as_str()) {
                all_resolved &= self.check_flag(claim, flag);
            }
        }
        self.check_reference_verb(claim);
        if all_resolved {
            self.coverage.resolved_command_references =
                self.coverage.resolved_command_references.saturating_add(1);
        }
    }

    /// Checks one declared flag and its policy standing.
    fn check_flag(&mut self, claim: ClaimRef<'_>, flag: &str) -> bool {
        let accepted = self
            .index
            .command(claim.path)
            .is_some_and(|command| command.accepts(flag));
        if !accepted {
            self.push_structural(
                claim,
                FindingData {
                    code: codes::UNKNOWN_FLAG,
                    option: Some(flag.to_owned()),
                    message: format!(
                        "flag '{flag}' is not accepted by '{}'",
                        join_path(claim.path)
                    ),
                },
            );
            return false;
        }
        self.push_advice(claim, flag_name_advice(self.policy, claim.path, flag));
        true
    }

    /// Applies policy advice to the resolved command's declared verb.
    fn check_reference_verb(&mut self, claim: ClaimRef<'_>) {
        let canonical_verb = self
            .index
            .command(claim.path)
            .and_then(|command| command.canonical_verb.clone());
        if let Some(verb) = canonical_verb {
            self.push_advice(claim, verb_advice(self.policy, claim.path, &verb));
        }
    }

    /// Appends a structural finding anchored to `claim`.
    fn push_structural(&mut self, claim: ClaimRef<'_>, data: FindingData) {
        self.findings.push((
            claim.order,
            ValidationFinding {
                category: FindingCategory::Structural,
                code: data.code.to_owned(),
                severity: crate::policy::PolicySeverity::Deny,
                manifest_id: claim.manifest_id.to_owned(),
                source_ordinal: claim.source_ordinal,
                command_path: claim.path.to_vec(),
                option: data.option,
                message: data.message,
            },
        ));
    }

    /// Appends policy advice, when there is any.
    fn push_advice(&mut self, claim: ClaimRef<'_>, advice: Option<PolicyAdvice>) {
        let Some(policy_advice) = advice else { return };
        self.findings.push((
            claim.order,
            ValidationFinding {
                category: FindingCategory::Policy,
                code: policy_advice.code.to_owned(),
                severity: self.policy.severity(),
                manifest_id: claim.manifest_id.to_owned(),
                source_ordinal: claim.source_ordinal,
                command_path: claim.path.to_vec(),
                option: policy_advice.option,
                message: policy_advice.message,
            },
        ));
    }
}

/// Returns the reference at a one-based ordinal.
fn reference_at(manifest: &ManifestClaims, ordinal: usize) -> Option<&SkillCommandRef> {
    ordinal
        .checked_sub(1)
        .and_then(|index| manifest.commands.get(index))
}

/// Counts examples with one extraction state.
fn count_extraction(input: &SkillValidationInput, wanted: ExampleExtraction) -> usize {
    input
        .examples
        .iter()
        .filter(|example| example.extraction == wanted)
        .count()
}

/// Validates supplied skill claims against an explicit command surface.
///
/// # Errors
///
/// Returns [`CommandSurfaceError`] when the surface is ambiguous: an empty
/// path, a duplicate path, an empty option name, or one option name carrying
/// conflicting metadata. No claim is evaluated in that case.
///
/// # Examples
///
/// ```rust
/// use cargo_orthohelp::skill_validation::{
///     EffectiveCommandSurface, ResolvedAgentPolicy, SkillValidationInput, validate,
/// };
/// use cargo_orthohelp::policy::PolicyMode;
///
/// let policy = ResolvedAgentPolicy {
///     mode: PolicyMode::Off,
///     exceptions: Vec::new(),
/// };
/// let report = validate(
///     &SkillValidationInput::default(),
///     &EffectiveCommandSurface::default(),
///     &policy,
/// )
/// .expect("empty input is valid");
/// assert!(report.findings.is_empty());
/// ```
pub fn validate(
    input: &SkillValidationInput,
    surface: &EffectiveCommandSurface,
    policy: &ResolvedAgentPolicy,
) -> Result<SkillValidationReport, CommandSurfaceError> {
    let index = CommandIndex::build(surface)?;
    let validator = Validator {
        index,
        manifests: ManifestIndex::new(&input.manifests),
        policy,
        findings: Vec::new(),
        coverage: Coverage::default(),
    };
    Ok(validator.run(input))
}
