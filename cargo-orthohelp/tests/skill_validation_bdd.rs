//! Behavioural scenarios for skill-manifest validation.
//!
//! These scenarios exercise the validator's consumer-facing contract without
//! touching the filesystem or a real command: every `Given` builds supplied
//! claims and an explicit effective command surface, and every `When` runs one
//! `validate` call whose single report the `Then` steps read. File parsing and
//! the fixture round trip are covered separately in `skill_validation_e2e`.
//!
//! Scenario state and the fixture builders live here; the step definitions live
//! in the `steps` submodule. `rstest-bdd` discovers steps through `inventory`,
//! so a step compiled into this binary is visible to these scenarios whichever
//! module defines it.

use cargo_orthohelp::skill_validation::{
    CommandContract, EffectiveCommandSurface, EffectiveOption, ExampleExtraction, ManifestClaims,
    NormalizedExample, OptionUse, ResolvedAgentPolicy, SkillValidationInput, SkillValidationReport,
    codes,
};
use ortho_config::SkillCommandRef;
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, scenarios};

/// The crate root's children live beside it, so the step module needs an
/// explicit path to stay colocated with this file's own directory.
#[path = "skill_validation_bdd/steps.rs"]
mod steps;

/// Error type for step definition failures.
pub(crate) type StepError = Box<dyn std::error::Error + Send + Sync>;

/// Result type for step definition operations.
pub(crate) type StepResult<T> = Result<T, StepError>;

/// Scenario state accumulated by the steps.
#[derive(Debug, Default, ScenarioState)]
struct SkillContext {
    /// Explicit effective command surface, built up by `Given` steps.
    pub(crate) surface: Slot<EffectiveCommandSurface>,
    /// Supplied manifest claims and examples, built up by `Given` steps.
    pub(crate) input: Slot<SkillValidationInput>,
    /// Policy configuration for the run.
    pub(crate) policy: Slot<ResolvedAgentPolicy>,
    /// The single report produced by the `When` step.
    pub(crate) report: Slot<SkillValidationReport>,
}

/// Creates the empty scenario state for each scenario.
/// `rstest-bdd` injects a fixture into a step whose parameter name matches it,
/// so the fixture is named `context` to match every step.
#[fixture]
fn context() -> SkillContext {
    SkillContext::default()
}

scenarios!(
    "tests/features/skill_manifest_validation.feature",
    fixtures = [context: SkillContext]
);

/// Builds one domain option with no declared value type.
pub(crate) fn option(long: &str) -> EffectiveOption {
    EffectiveOption {
        long: long.to_owned(),
        value_type: None,
        is_global: false,
    }
}

/// Builds one global option, as a consumer would classify it.
pub(crate) fn global(long: &str) -> EffectiveOption {
    EffectiveOption {
        is_global: true,
        ..option(long)
    }
}

/// Builds one command contract whose listed options are global options.
pub(crate) fn command_with_globals(
    segments: &[&str],
    options: &[&str],
    globals: &[&str],
) -> CommandContract {
    CommandContract {
        options: options
            .iter()
            .map(|name| {
                if globals.contains(name) {
                    global(name)
                } else {
                    option(name)
                }
            })
            .collect(),
        ..command(segments, &[])
    }
}

/// Builds one command contract from bare option names.
pub(crate) fn command(segments: &[&str], options: &[&str]) -> CommandContract {
    CommandContract {
        path: segments
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect(),
        options: options.iter().map(|long| option(long)).collect(),
        canonical_verb: None,
    }
}

/// Appends one command to the scenario's effective surface.
pub(crate) fn add_command(context: &SkillContext, contract: CommandContract) {
    let mut surface = context
        .surface
        .get_or_insert_with(EffectiveCommandSurface::default);
    surface.commands.push(contract);
}

/// Appends one option name to the last command on the surface.
pub(crate) fn add_option_to_last_command(context: &SkillContext, long: &str) -> StepResult<()> {
    push_option_on_last_command(context, option(long))
}

/// Appends one global option to the last declared command.
pub(crate) fn add_global_option_to_last_command(
    context: &SkillContext,
    long: &str,
) -> StepResult<()> {
    push_option_on_last_command(context, global(long))
}

/// Appends one prepared option to the last declared command.
fn push_option_on_last_command(
    context: &SkillContext,
    prepared: EffectiveOption,
) -> StepResult<()> {
    let mut surface = context
        .surface
        .get_or_insert_with(EffectiveCommandSurface::default);
    let command = surface
        .commands
        .last_mut()
        .ok_or("no command has been declared")?;
    command.options.push(prepared);
    Ok(())
}

/// Appends one manifest to the scenario's claim input.
pub(crate) fn add_manifest(context: &SkillContext, manifest: ManifestClaims) {
    let mut input = context
        .input
        .get_or_insert_with(SkillValidationInput::default);
    input.manifests.push(manifest);
}

/// Appends one command reference to the last declared manifest.
pub(crate) fn add_reference(context: &SkillContext, reference: SkillCommandRef) -> StepResult<()> {
    let mut input = context
        .input
        .get_or_insert_with(SkillValidationInput::default);
    let manifest = input
        .manifests
        .last_mut()
        .ok_or("no manifest has been declared")?;
    manifest.commands.push(reference);
    Ok(())
}

/// Appends one example to the scenario's claim input.
pub(crate) fn add_example(context: &SkillContext, example: NormalizedExample) {
    let mut input = context
        .input
        .get_or_insert_with(SkillValidationInput::default);
    input.examples.push(example);
}

/// Builds one command reference.
pub(crate) fn reference(segments: &[&str], flags: &[&str]) -> SkillCommandRef {
    SkillCommandRef {
        path: segments
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect(),
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
    }
}

/// Builds one example bound to reference one.
pub(crate) fn example(
    manifest_id: &str,
    segments: &[&str],
    options: Vec<OptionUse>,
) -> NormalizedExample {
    NormalizedExample {
        manifest_id: manifest_id.to_owned(),
        command_reference_ordinal: 1,
        path: segments
            .iter()
            .map(|segment| (*segment).to_owned())
            .collect(),
        options,
        positionals: Vec::new(),
        source_ordinal: 1,
        extraction: ExampleExtraction::default(),
    }
}

/// Builds one option use.
pub(crate) fn option_use(name: &str, value: Option<&str>, source_ordinal: usize) -> OptionUse {
    OptionUse {
        name: name.to_owned(),
        value: value.map(str::to_owned),
        source_ordinal,
    }
}

/// The feature file quotes finding codes and options as literals; this test
/// keeps the codes tied to the exported constants.
#[test]
fn finding_codes_match_the_feature_file() {
    assert_eq!(codes::UNKNOWN_FLAG, "skill.unknown_flag");
    assert_eq!(
        codes::EXAMPLE_UNKNOWN_OPTION,
        "skill.example.unknown_option"
    );
    assert_eq!(
        codes::POLICY_GLOBAL_OPTION_NEAR_MISS,
        "skill.policy.global_option_near_miss"
    );
}
