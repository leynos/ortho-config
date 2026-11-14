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

use proptest::prelude::*;

use super::input::{ExampleExtraction, NormalizedExample, OptionUse, SkillValidationInput};
use super::support::{command, input, manifest, reference, surface};

/// Fixed command paths the generator may declare or reference.
const PATHS: &[&[&str]] = &[&["tool"], &["tool", "alpha"], &["tool", "beta"]];

/// Option names a command or a command reference may declare.
const DECLARED: &[&str] = &["json", "color", "bogus"];

/// Option names an example may use. The last is never declared anywhere.
const NAMES: &[&str] = &["json", "color", "bogus", "extra"];

/// Opaque values an example may carry, including hostile and empty shapes.
const VALUES: &[&str] = &["always", "with space", "-x", "s3krit"];

/// Positional arguments an example may carry, including hostile shapes.
const POSITIONALS: &[&str] = &["input.txt", "--literal", "with space"];

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
    /// Positional arguments the example shows, preserved verbatim.
    positionals: Vec<String>,
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
    let extra_positionals = prop::collection::vec(prop::sample::select(POSITIONALS), 0..=2);
    (
        0..2u8,
        prop::bool::ANY,
        0..=3usize,
        0..PATHS.len(),
        uses,
        extra_positionals,
    )
        .prop_map(
            |(manifest, is_extracted, ordinal, path, options, positionals)| Spec {
                manifest,
                is_extracted,
                ordinal,
                path,
                options: options
                    .into_iter()
                    .map(|(name, value)| (name, value.map(str::to_owned)))
                    .collect(),
                positionals: positionals.into_iter().map(str::to_owned).collect(),
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
        positionals: spec.positionals.clone(),
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

mod oracle;
mod properties;
