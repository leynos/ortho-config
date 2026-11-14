//! Property tests for normalized-example binding and value preservation.
//!
//! The generator walks the whole binding chain: an example may name a manifest
//! that is absent, repeated, or unique; its ordinal may or may not select a
//! reference; its path may or may not equal that reference's path; and each of
//! its option names may be undeclared, declared but unaccepted, or accepted.
//! The oracle here re-derives the expected findings with its own loops rather
//! than calling the validator's index or resolution code.
//!
//! The generated properties and their mutant controls live in the `properties`
//! child module. Controls mutate a candidate rather than the validator: one
//! drops a value from a normalized record, and one unions the root's options
//! into a child. Each control fails the comparison it guards, so neither
//! comparison can pass vacuously.

use std::collections::BTreeSet;

use proptest::prelude::*;

use super::input::{ExampleExtraction, NormalizedExample, OptionUse, SkillValidationInput};
use super::report::{SkillValidationReport, codes};
use super::support::{command, input, manifest, reference, surface};

/// Fixed command paths the generator may declare or reference.
const PATHS: &[&[&str]] = &[&["tool"], &["tool", "alpha"], &["tool", "beta"]];

/// Option names a command or a command reference may declare.
const DECLARED: &[&str] = &["json", "color", "bogus"];

/// Option names an example may use. The last is never declared anywhere.
const NAMES: &[&str] = &["json", "color", "bogus", "extra"];

/// Opaque values an example may carry, including hostile and empty shapes.
const VALUES: &[&str] = &["always", "with space", "-x", "s3krit"];

/// One generated example claim.
#[derive(Debug, Clone)]
struct Spec {
    /// Manifest ID index the example names.
    manifest: u8,
    /// Whether the consumer extracted the example completely.
    is_extracted: bool,
    /// One-based reference ordinal the example claims; zero selects none.
    ordinal: usize,
    /// Path index the example claims.
    path: usize,
    /// Option uses in source order, as name index and optional value.
    options: Vec<(usize, Option<String>)>,
}

/// One generated validation case.
#[derive(Debug, Clone)]
struct Case {
    /// Declared paths, each with its option bitmask.
    surface: Vec<(usize, u8)>,
    /// Manifests as an ID index plus references of path index and option mask.
    manifests: Vec<(u8, Vec<(usize, u8)>)>,
    /// Examples in source order.
    examples: Vec<Spec>,
}

/// Generates one example claim.
fn spec_strategy() -> impl Strategy<Value = Spec> {
    let uses = prop::collection::vec(
        (
            0..NAMES.len(),
            prop::option::of(prop::sample::select(VALUES)),
        ),
        0..=3,
    );
    (0..2u8, prop::bool::ANY, 0..=3usize, 0..PATHS.len(), uses).prop_map(
        |(manifest, is_extracted, ordinal, path, options)| Spec {
            manifest,
            is_extracted,
            ordinal,
            path,
            options: options
                .into_iter()
                .map(|(name, value)| (name, value.map(str::to_owned)))
                .collect(),
        },
    )
}

/// Generates varied surfaces, claims, and examples.
fn case_strategy() -> impl Strategy<Value = Case> {
    let masks = prop::collection::vec(0..8u8, PATHS.len());
    let references = prop::collection::vec((0..PATHS.len(), 0..8u8), 0..=3);
    let manifests = prop::collection::vec((0..2u8, references), 0..=3);
    let examples = prop::collection::vec(spec_strategy(), 0..=3);

    (masks, manifests, examples).prop_map(|(surface_masks, claims, specs)| Case {
        surface: surface_masks
            .into_iter()
            .enumerate()
            .filter(|(_, mask)| *mask != 0)
            .collect(),
        manifests: claims,
        examples: specs,
    })
}

/// Returns the path at `index`, falling back to the root command.
///
/// The fallback is unreachable for generated indices; returning a value keeps
/// the helper total without an `expect` outside a test assertion.
fn segments(index: usize) -> &'static [&'static str] {
    PATHS.get(index).copied().unwrap_or(&["tool"])
}

/// Returns the surface option mask for one path, if declared.
fn declared_mask(case: &Case, path: usize) -> Option<u8> {
    case.surface
        .iter()
        .find(|(index, _)| *index == path)
        .map(|(_, mask)| *mask)
}

/// Expands a reference's option bitmask into ascending names.
fn declared_names(mask: u8) -> Vec<&'static str> {
    DECLARED
        .iter()
        .enumerate()
        .filter(|(bit, _)| mask & (1 << bit) != 0)
        .map(|(_, name)| *name)
        .collect()
}

/// Builds one normalized example from a generated claim.
fn build_example(spec: &Spec, position: usize, manifest_id: &str) -> NormalizedExample {
    NormalizedExample {
        manifest_id: manifest_id.to_owned(),
        command_reference_ordinal: spec.ordinal,
        path: segments(spec.path)
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect(),
        options: spec
            .options
            .iter()
            .enumerate()
            .map(|(index, (name, value))| OptionUse {
                name: NAMES.get(*name).copied().unwrap_or("json").to_owned(),
                value: value.clone(),
                source_ordinal: index.saturating_add(1),
            })
            .collect(),
        positionals: Vec::new(),
        source_ordinal: position.saturating_add(1),
        extraction: if spec.is_extracted {
            ExampleExtraction::Checked
        } else {
            ExampleExtraction::Unsupported
        },
    }
}

impl Case {
    /// Builds the effective command surface this case describes.
    fn to_surface(&self) -> super::input::EffectiveCommandSurface {
        let commands = self
            .surface
            .iter()
            .map(|(path_index, mask)| command(segments(*path_index), &declared_names(*mask)))
            .collect();
        surface(commands)
    }

    /// Builds the claim input this case describes.
    fn to_input(&self) -> SkillValidationInput {
        let manifests = self
            .manifests
            .iter()
            .map(|(id_index, references)| {
                let claims = references
                    .iter()
                    .map(|(path_index, mask)| {
                        reference(segments(*path_index), &declared_names(*mask))
                    })
                    .collect();
                manifest(&format!("m{id_index}"), claims)
            })
            .collect();
        let examples = self
            .examples
            .iter()
            .enumerate()
            .map(|(position, spec)| build_example(spec, position, &format!("m{}", spec.manifest)))
            .collect();
        input(manifests, examples)
    }
}

/// One expected example finding.
struct Expected {
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
struct ExpectedCoverage {
    /// Examples that reached their option checks.
    bound: usize,
    /// Examples the consumer extracted completely.
    checked: usize,
    /// Examples the consumer could not extract.
    unchecked: usize,
}

/// The oracle's accumulated expectations.
#[derive(Default)]
struct Oracle {
    /// Expected findings in claim order.
    findings: Vec<Expected>,
    /// Expected coverage.
    coverage: ExpectedCoverage,
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
        let Some((reference_path, declared)) = spec
            .ordinal
            .checked_sub(1)
            .and_then(|index| bound.references.get(index))
        else {
            self.findings
                .push(bound.anchor.finding(codes::EXAMPLE_UNBOUND_REFERENCE, None));
            return;
        };
        if *reference_path != spec.path {
            self.findings
                .push(bound.anchor.finding(codes::EXAMPLE_PATH_MISMATCH, None));
            return;
        }
        self.coverage.bound = self.coverage.bound.saturating_add(1);
        // Ownership is checked before reality, and the two are not symmetric: an
        // option the reference never declared is reported even when the path is
        // absent from the surface. Only a *declared* option needs the surface,
        // and a path the surface does not declare stops the remaining checks
        // rather than cascading.
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (name_index, _) in &spec.options {
            let name = NAMES.get(*name_index).copied().unwrap_or("json");
            if !seen.insert(name) {
                continue;
            }
            let bit = 1u8 << name_index;
            let is_declared = *name_index < DECLARED.len() && declared & bit != 0;
            let code = if is_declared {
                match bound.accepted {
                    None => return,
                    Some(known) if known & bit == 0 => Some(codes::EXAMPLE_UNKNOWN_OPTION),
                    Some(_) => None,
                }
            } else {
                Some(codes::EXAMPLE_UNDECLARED_OPTION)
            };
            if let Some(finding_code) = code {
                self.findings
                    .push(bound.anchor.finding(finding_code, Some(name)));
            }
        }
    }
}

/// Re-derives the expected findings, coverage, and status for one case.
fn oracle(case: &Case) -> (Vec<Expected>, ExpectedCoverage) {
    let mut oracle = Oracle::default();
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
fn example_comparables(
    report: &SkillValidationReport,
) -> Vec<(String, usize, String, Option<String>)> {
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

mod properties;
