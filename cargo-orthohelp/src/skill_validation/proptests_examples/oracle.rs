//! The independent linear oracle for generated example claims.
//!
//! The oracle re-derives the expected findings, coverage, and status with its
//! own loops. It deliberately duplicates the validator's rules instead of
//! calling its index, resolution, or policy code, so a mistake in the
//! implementation cannot also define the expectation it is compared against.

use std::collections::BTreeSet;

use super::super::report::{SkillValidationReport, codes};
use super::{Case, DECLARED, NAMES, Spec, declared_mask};

/// A finding reduced to the fields the tests compare.
pub(super) type Comparable = (String, usize, String, Option<String>);

/// One expected example finding.
pub(super) struct Expected {
    /// Position of the owning manifest, or `usize::MAX` when none owns it.
    order: usize,
    /// Manifest ID the example named.
    manifest_id: String,
    /// One-based source ordinal of the example.
    source_ordinal: usize,
    /// Expected finding code.
    code: &'static str,
    /// Expected option name, when the code carries one.
    option: Option<String>,
}

/// Where one example's findings are anchored.
#[derive(Clone)]
struct Anchor {
    /// Position of the owning manifest, or `usize::MAX` when none owns it.
    order: usize,
    /// Manifest ID the example named.
    manifest_id: String,
    /// One-based source ordinal of the example.
    source_ordinal: usize,
}

impl Anchor {
    /// Anchors an expectation to one example.
    fn new(order: usize, manifest_id: &str, source_ordinal: usize) -> Self {
        Self {
            order,
            manifest_id: manifest_id.to_owned(),
            source_ordinal,
        }
    }

    /// Builds one expected finding at this anchor.
    fn finding(&self, code: &'static str, option: Option<&str>) -> Expected {
        Expected {
            order: self.order,
            manifest_id: self.manifest_id.clone(),
            source_ordinal: self.source_ordinal,
            code,
            option: option.map(str::to_owned),
        }
    }
}

/// One example whose owner resolved uniquely, plus what the surface says.
struct Bound<'a> {
    /// Where this example's findings are anchored.
    anchor: &'a Anchor,
    /// The owning manifest's command references.
    references: &'a [(usize, u8)],
    /// The surface's option mask for the example's path, when declared.
    accepted: Option<u8>,
}

/// Coverage the oracle predicts.
#[derive(Debug, Default)]
pub(super) struct ExpectedCoverage {
    /// Examples that reached their option checks.
    pub(super) bound: usize,
    /// Examples the consumer extracted completely.
    pub(super) checked: usize,
    /// Examples the consumer could not extract.
    pub(super) unchecked: usize,
}

/// How the oracle treats the surface's acceptance of a declared option.
#[derive(Clone, Copy, Default)]
enum Acceptance {
    /// Only an accepted option is silent; a declared but unaccepted one is
    /// reported.
    #[default]
    Honour,
    /// Treat every declared option as accepted. This is the mutant the
    /// acceptance control compares against, so the rule has a witness on the
    /// same input rather than only across different surfaces.
    Ignore,
}

/// The oracle's accumulated expectations.
#[derive(Default)]
struct Oracle {
    /// Expected findings in claim order.
    findings: Vec<Expected>,
    /// Expected coverage.
    coverage: ExpectedCoverage,
    /// How acceptance is treated.
    acceptance: Acceptance,
}

impl Oracle {
    /// Predicts one example's findings and coverage contribution.
    fn expect(&mut self, case: &Case, spec: &Spec, source_ordinal: usize) {
        if !spec.is_extracted {
            self.coverage.unchecked = self.coverage.unchecked.saturating_add(1);
            return;
        }
        self.coverage.checked = self.coverage.checked.saturating_add(1);
        let manifest_id = format!("m{}", spec.manifest);
        let owners: Vec<(usize, &[(usize, u8)])> = case
            .manifests
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.0 == spec.manifest)
            .map(|(order, entry)| (order, entry.1.as_slice()))
            .collect();

        match owners.as_slice() {
            [] => {
                let anchor = Anchor::new(usize::MAX, &manifest_id, source_ordinal);
                self.findings
                    .push(anchor.finding(codes::EXAMPLE_UNKNOWN_MANIFEST, None));
            }
            [(order, _), _, ..] => {
                let anchor = Anchor::new(*order, &manifest_id, source_ordinal);
                self.findings
                    .push(anchor.finding(codes::EXAMPLE_AMBIGUOUS_MANIFEST, None));
            }
            [(order, references)] => {
                let anchor = Anchor::new(*order, &manifest_id, source_ordinal);
                let bound = Bound {
                    anchor: &anchor,
                    references,
                    accepted: declared_mask(case, spec.path),
                };
                self.expect_bound(spec, &bound);
            }
        }
    }

    /// Predicts the findings of an example whose owner resolved uniquely.
    fn expect_bound(&mut self, spec: &Spec, bound: &Bound<'_>) {
        let declared = match bound_reference(spec, bound) {
            Ok(declared) => declared,
            Err(code) => {
                self.findings.push(bound.anchor.finding(code, None));
                return;
            }
        };
        let rules = OptionRules {
            declared,
            accepted: bound.accepted,
            acceptance: self.acceptance,
        };
        if self.expect_options(spec, bound, rules) {
            self.coverage.bound = self.coverage.bound.saturating_add(1);
        }
    }

    /// Predicts each distinct option's finding in source order.
    ///
    /// Returns whether every option was checked. An example counts as bound
    /// only once all of them were, so a path the surface does not declare
    /// leaves it unbound rather than claiming a validation that never ran.
    /// Ownership is checked before reality, and the two are not symmetric: an
    /// option the reference never declared is reported even when the path is
    /// absent from the surface. Only a *declared* option needs the surface, and
    /// a path the surface does not declare stops the remaining checks rather
    /// than cascading.
    fn expect_options(&mut self, spec: &Spec, bound: &Bound<'_>, rules: OptionRules) -> bool {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let steps = spec
            .options
            .iter()
            .filter_map(|(index, _)| distinct_step(*index, rules, &mut seen));
        for (name, step) in steps {
            match step {
                Step::Stop => return false,
                Step::Accept => {}
                Step::Report(code) => self.findings.push(bound.anchor.finding(code, Some(name))),
            }
        }
        // A path the surface lacks leaves the example unbound even when it used
        // no declared option: nothing about that path was checked.
        bound.accepted.is_some()
    }
}

/// Resolves the command reference an example selects.
///
/// Returns the reference's declared flag mask, or the code to report when the
/// ordinal selects no reference or selects one for a different path.
fn bound_reference(spec: &Spec, bound: &Bound<'_>) -> Result<u8, &'static str> {
    let Some((reference_path, declared)) = spec
        .ordinal
        .checked_sub(1)
        .and_then(|index| bound.references.get(index))
    else {
        return Err(codes::EXAMPLE_UNBOUND_REFERENCE);
    };
    if *reference_path == spec.path {
        return Ok(*declared);
    }
    Err(codes::EXAMPLE_PATH_MISMATCH)
}

/// What the oracle knows about one example's reference and its surface.
#[derive(Clone, Copy)]
struct OptionRules {
    /// Flags the owning reference declares.
    declared: u8,
    /// Options the surface accepts, or `None` when it lacks the example's path.
    accepted: Option<u8>,
    /// How acceptance is treated.
    acceptance: Acceptance,
}

/// What checking one distinct example option implies.
enum Step {
    /// The surface lacks the example's path, so no later option is checkable.
    Stop,
    /// The option is declared and accepted, so it produces no finding.
    Accept,
    /// The option is reported with the supplied code.
    Report(&'static str),
}

/// Classifies the first occurrence of one example option.
///
/// Returns the option's name with the step it implies, or `None` when the name
/// repeats one already classified, because one option produces at most one
/// finding.
fn distinct_step<'a>(
    name_index: usize,
    rules: OptionRules,
    seen: &mut BTreeSet<&'a str>,
) -> Option<(&'a str, Step)> {
    let name = NAMES.get(name_index).copied()?;
    if !seen.insert(name) {
        return None;
    }
    Some((name, classify_option(name_index, rules)))
}

/// Classifies one distinct option against its reference and the surface.
fn classify_option(name_index: usize, rules: OptionRules) -> Step {
    let bit = 1u8 << name_index;
    let is_declared = name_index < DECLARED.len() && rules.declared & bit != 0;
    if !is_declared {
        return Step::Report(codes::EXAMPLE_UNDECLARED_OPTION);
    }
    let accepted = match rules.acceptance {
        Acceptance::Honour => rules.accepted,
        Acceptance::Ignore => Some(u8::MAX),
    };
    match accepted {
        None => Step::Stop,
        Some(known) if known & bit == 0 => Step::Report(codes::EXAMPLE_UNKNOWN_OPTION),
        Some(_) => Step::Accept,
    }
}

/// Re-derives the expected findings, coverage, and status for one case.
pub(super) fn oracle(case: &Case) -> (Vec<Expected>, ExpectedCoverage) {
    oracle_with(case, Acceptance::Honour)
}

/// Predicts the same case as if the surface accepted every declared option.
///
/// This exists only as a control: a comparison that still agrees after this
/// mutation cannot be detecting the acceptance rule.
#[cfg(test)]
pub(super) fn oracle_ignoring_acceptance(case: &Case) -> (Vec<Expected>, ExpectedCoverage) {
    oracle_with(case, Acceptance::Ignore)
}

/// Re-derives one case under the supplied acceptance policy.
fn oracle_with(case: &Case, acceptance: Acceptance) -> (Vec<Expected>, ExpectedCoverage) {
    let mut oracle = Oracle {
        acceptance,
        ..Oracle::default()
    };
    for (position, spec) in case.examples.iter().enumerate() {
        oracle.expect(case, spec, position.saturating_add(1));
    }
    // The validator's sort is stable, and expectations were pushed in the same
    // order, so equal keys keep the same relative order on both sides.
    oracle.findings.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then(left.source_ordinal.cmp(&right.source_ordinal))
            .then(left.code.cmp(right.code))
    });
    (oracle.findings, oracle.coverage)
}

/// Reduces a report to the example findings the oracle predicts.
pub(super) fn example_comparables(report: &SkillValidationReport) -> Vec<Comparable> {
    report
        .findings
        .iter()
        .filter(|finding| finding.code.starts_with("skill.example."))
        .map(|finding| {
            (
                finding.manifest_id.clone(),
                finding.source_ordinal,
                finding.code.clone(),
                finding.option.clone(),
            )
        })
        .collect()
}

/// Reduces the oracle's predictions to the same comparable shape.
pub(super) fn expected_comparables(findings: &[Expected]) -> Vec<Comparable> {
    findings
        .iter()
        .map(|finding| {
            (
                finding.manifest_id.clone(),
                finding.source_ordinal,
                finding.code.to_owned(),
                finding.option.clone(),
            )
        })
        .collect()
}
