//! End-to-end fixture round trip for skill-manifest validation.
//!
//! The fixtures under `cargo-orthohelp/tests/fixtures/skills` are real Markdown
//! documents with two fenced JSON blocks each: `## Command index` holds the
//! manifest's command references and `## Checked examples` holds its normalized
//! examples. This adapter is the only Markdown reader in the feature, which
//! keeps the library itself free of file and prose parsing.
//!
//! The drift test mutates the fixture *source* and re-parses it, so an adapter
//! that returned stale or hardcoded records would fail the extracted-record
//! assertion before the finding assertion is reached.

use std::error::Error;

use camino::Utf8PathBuf;
use cap_std::ambient_authority;
use cap_std::fs_utf8::Dir;
use cargo_orthohelp::policy::PolicyMode;
use cargo_orthohelp::skill_validation::{
    EffectiveCommandSurface, ManifestClaims, NormalizedExample, ResolvedAgentPolicy,
    SkillValidationInput, SkillValidationReport, codes, validate,
};
use rstest::rstest;

/// Error type for the fixture helpers.
type TestError = Box<dyn Error>;

/// Directory holding the fixtures, relative to the crate manifest.
const FIXTURE_SUBDIR: &str = "tests/fixtures/skills";

/// Heading introducing the command-reference array.
const COMMAND_INDEX_HEADING: &str = "## Command index";

/// Heading introducing the normalized-example array.
const CHECKED_EXAMPLES_HEADING: &str = "## Checked examples";

/// Returns the absolute fixture root.
fn fixture_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_SUBDIR)
}

/// Opens the fixture root as a capability handle.
///
/// The ambient authority is scoped to the checked-in fixture directory, so the
/// adapter can read fixtures without any other part of the filesystem becoming
/// reachable from the test.
fn fixture_dir() -> Result<Dir, TestError> {
    Ok(Dir::open_ambient_dir(fixture_root(), ambient_authority())?)
}

/// Returns the JSON body of the single fenced block after `heading`.
fn fenced_json_after<'a>(markdown: &'a str, heading: &str) -> Result<&'a str, String> {
    if markdown.matches(heading).count() != 1 {
        return Err(format!("heading `{heading}` must appear exactly once"));
    }
    let (_, after_heading) = markdown
        .split_once(heading)
        .ok_or_else(|| format!("missing heading `{heading}`"))?;
    let (_, after_fence) = after_heading
        .split_once("```json")
        .ok_or_else(|| format!("missing json block after `{heading}`"))?;
    let (body, _) = after_fence
        .split_once("```")
        .ok_or_else(|| format!("unterminated json block after `{heading}`"))?;
    Ok(body.trim())
}

/// One checked-in fixture directory under `tests/fixtures/skills`.
///
/// A fixture is named by its own directory, and its manifest ID is that same
/// name, so the type keeps the two from drifting apart.
#[derive(Clone, Copy)]
struct Fixture(&'static str);

impl Fixture {
    /// Reads this fixture's Markdown source.
    fn markdown(self) -> Result<String, TestError> {
        let relative = Utf8PathBuf::from(self.0).join("SKILL.md");
        Ok(fixture_dir()?.read_to_string(relative)?)
    }

    /// Parses this fixture's command claims out of its Markdown source.
    fn manifest(self, markdown: &str) -> Result<ManifestClaims, TestError> {
        let commands = serde_json::from_str(fenced_json_after(markdown, COMMAND_INDEX_HEADING)?)?;
        Ok(ManifestClaims {
            id: self.0.to_owned(),
            commands,
        })
    }

    /// Validates this fixture's claims against the shared command surface.
    fn validate(self) -> Result<SkillValidationReport, TestError> {
        let markdown = self.markdown()?;
        let input = SkillValidationInput {
            manifests: vec![self.manifest(&markdown)?],
            examples: parse_examples(&markdown)?,
        };
        Ok(validate(&input, &load_surface()?, &deny_policy())?)
    }
}

/// Parses normalized examples out of Markdown source.
fn parse_examples(markdown: &str) -> Result<Vec<NormalizedExample>, TestError> {
    Ok(serde_json::from_str(fenced_json_after(
        markdown,
        CHECKED_EXAMPLES_HEADING,
    )?)?)
}

/// Loads the shared effective command surface.
fn load_surface() -> Result<EffectiveCommandSurface, TestError> {
    Ok(serde_json::from_str(
        &fixture_dir()?.read_to_string("command-surface.json")?,
    )?)
}

/// Returns the deny-mode policy used by the clean fixtures.
const fn deny_policy() -> ResolvedAgentPolicy {
    ResolvedAgentPolicy {
        mode: PolicyMode::Deny,
        exceptions: Vec::new(),
    }
}

/// Returns the finding codes present in a report.
fn codes_of(report: &SkillValidationReport) -> Vec<&str> {
    report
        .findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect()
}

/// Returns the option names each example uses, in example order.
fn option_names(examples: &[NormalizedExample]) -> Vec<String> {
    examples
        .iter()
        .flat_map(|example| example.options.iter())
        .map(|option| option.name.clone())
        .collect()
}

#[rstest]
#[case(Fixture("operation"), 1)]
#[case(Fixture("build-workflow"), 2)]
fn a_supplied_fixture_validates_cleanly(#[case] fixture: Fixture, #[case] examples_checked: usize) {
    let report = fixture.validate().expect("the fixture validates");

    assert!(
        report.findings.is_empty(),
        "unexpected findings: {:?}",
        codes_of(&report)
    );
    assert_eq!(report.coverage.command_references, 1);
    assert_eq!(report.coverage.resolved_command_references, 1);
    assert_eq!(report.coverage.examples_checked, examples_checked);
    assert!(report.is_complete());
}

#[test]
fn source_drift_flows_into_the_extracted_record_and_the_report() {
    let fixture = Fixture("build-workflow");
    let markdown = fixture.markdown().expect("the fixture reads");
    let baseline = parse_examples(&markdown).expect("the baseline parses");
    assert_eq!(option_names(&baseline), vec!["json", "color"]);

    let drifted = markdown.replace("\"name\": \"json\"", "\"name\": \"missing-option\"");
    assert_ne!(drifted, markdown, "the fixture no longer contains `json`");

    let examples = parse_examples(&drifted).expect("the drifted examples parse");
    assert_eq!(
        option_names(&examples),
        vec!["missing-option", "color"],
        "the adapter must reflect the mutated source, not a stale record"
    );

    let input = SkillValidationInput {
        manifests: vec![
            fixture
                .manifest(&drifted)
                .expect("the drifted manifest parses"),
        ],
        examples,
    };
    let report = validate(
        &input,
        &load_surface().expect("the surface loads"),
        &deny_policy(),
    )
    .expect("the surface is unambiguous");
    let found = codes_of(&report);

    assert!(
        found.contains(&codes::EXAMPLE_UNDECLARED_OPTION),
        "missing undeclared-option finding: {found:?}"
    );
    assert!(
        found.contains(&codes::EXAMPLE_UNKNOWN_OPTION),
        "missing unknown-option finding: {found:?}"
    );
}

#[test]
fn a_repeated_heading_is_a_typed_fixture_error() {
    let markdown = format!("{CHECKED_EXAMPLES_HEADING}\n{CHECKED_EXAMPLES_HEADING}\n");
    let error = fenced_json_after(&markdown, CHECKED_EXAMPLES_HEADING)
        .expect_err("a repeated heading must fail");

    assert!(error.contains("exactly once"));
}

#[test]
fn a_malformed_array_is_a_typed_fixture_error() {
    let markdown = format!("{COMMAND_INDEX_HEADING}\n\n```json\n[{{\"path\": 1}}]\n```\n");
    let body = fenced_json_after(&markdown, COMMAND_INDEX_HEADING)
        .expect("the command index heading is present");
    let parsed: Result<Vec<ortho_config::SkillCommandRef>, _> = serde_json::from_str(body);

    assert!(parsed.is_err(), "a malformed array must not parse");
}
