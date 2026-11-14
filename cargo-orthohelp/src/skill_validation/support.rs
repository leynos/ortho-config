//! Shared builders for skill-validation unit tests.
//!
//! These helpers never read files or execute commands. They exist so each test
//! states only the claim shapes it is actually about.

use ortho_config::SkillCommandRef;
use pretty_assertions::assert_eq;

use crate::policy::PolicyMode;
use crate::policy::config::{ExceptionKind, PolicyException};

use super::input::{
    CommandContract, EffectiveCommandSurface, EffectiveOption, ExampleExtraction, ManifestClaims,
    NormalizedExample, OptionUse, SkillValidationInput,
};
use super::policy_check::ResolvedAgentPolicy;
use super::report::{SkillValidationReport, ValidationFinding};

/// Converts path segments into one owned invocation path.
pub(crate) fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_owned())
        .collect()
}

/// Builds one effective option with no declared value type.
pub(crate) fn option(long: &str) -> EffectiveOption {
    EffectiveOption {
        long: long.to_owned(),
        value_type: None,
    }
}

/// Builds one command contract from bare option names.
pub(crate) fn command(segments: &[&str], options: &[&str]) -> CommandContract {
    CommandContract {
        path: path(segments),
        options: options.iter().map(|name| option(name)).collect(),
        canonical_verb: None,
    }
}

/// Builds one command contract with a declared verb.
pub(crate) fn command_with_verb(
    segments: &[&str],
    options: &[&str],
    verb: &str,
) -> CommandContract {
    CommandContract {
        canonical_verb: Some(verb.to_owned()),
        ..command(segments, options)
    }
}

/// Wraps command contracts into a surface.
pub(crate) fn surface(commands: Vec<CommandContract>) -> EffectiveCommandSurface {
    EffectiveCommandSurface { commands }
}

/// Builds one command reference.
pub(crate) fn reference(segments: &[&str], flags: &[&str]) -> SkillCommandRef {
    SkillCommandRef {
        path: path(segments),
        flags: flags.iter().map(|flag| (*flag).to_owned()).collect(),
    }
}

/// Builds one manifest claim.
pub(crate) fn manifest(id: &str, commands: Vec<SkillCommandRef>) -> ManifestClaims {
    ManifestClaims {
        id: id.to_owned(),
        commands,
    }
}

/// Wraps manifests and examples into an input.
pub(crate) fn input(
    manifests: Vec<ManifestClaims>,
    examples: Vec<NormalizedExample>,
) -> SkillValidationInput {
    SkillValidationInput {
        manifests,
        examples,
    }
}

/// Builds one example option use.
pub(crate) fn option_use(name: &str, value: Option<&str>, source_ordinal: usize) -> OptionUse {
    OptionUse {
        name: name.to_owned(),
        value: value.map(str::to_owned),
        source_ordinal,
    }
}

/// Builds a checked example whose source ordinal matches its reference ordinal.
pub(crate) fn example(
    manifest_id: &str,
    reference_ordinal: usize,
    segments: &[&str],
    options: Vec<OptionUse>,
) -> NormalizedExample {
    NormalizedExample {
        manifest_id: manifest_id.to_owned(),
        command_reference_ordinal: reference_ordinal,
        path: path(segments),
        options,
        positionals: Vec::new(),
        source_ordinal: reference_ordinal,
        extraction: ExampleExtraction::Checked,
    }
}

/// Validates one manifest's references against a single `tool` command that
/// declares `json`, under `mode`.
///
/// Most structural tests need exactly that arrangement, so it lives here
/// rather than being restated by each of them.
pub(crate) fn validate_tool_references(
    references: Vec<SkillCommandRef>,
    mode: PolicyMode,
) -> SkillValidationReport {
    let commands = surface(vec![command(&["tool"], &["json"])]);
    let claims = input(vec![manifest("operation-skill", references)], Vec::new());
    super::validate(&claims, &commands, &policy(mode)).expect("the surface is unambiguous")
}

/// Builds a policy with the supplied mode and no exceptions.
pub(crate) fn policy(mode: PolicyMode) -> ResolvedAgentPolicy {
    ResolvedAgentPolicy {
        mode,
        exceptions: Vec::new(),
    }
}

/// Builds a flag exception with an optional space-separated command scope.
pub(crate) fn flag_exception(name: &str, scope: Option<&str>) -> PolicyException {
    PolicyException {
        kind: ExceptionKind::Flag,
        name: name.to_owned(),
        reason: "test exception".to_owned(),
        command_path: scope.map(str::to_owned),
    }
}

/// Builds a verb exception with an optional space-separated command scope.
pub(crate) fn verb_exception(name: &str, scope: Option<&str>) -> PolicyException {
    PolicyException {
        kind: ExceptionKind::Verb,
        name: name.to_owned(),
        reason: "test exception".to_owned(),
        command_path: scope.map(str::to_owned),
    }
}

/// Returns the finding codes in report order.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(codes_of(&report), vec!["skill.unknown_flag"]);
/// ```
pub(crate) fn codes_of(report: &SkillValidationReport) -> Vec<&str> {
    report
        .findings
        .iter()
        .map(|finding| finding.code.as_str())
        .collect()
}

/// Returns the only finding, failing the caller when the count differs.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(only(&report).code, "skill.unknown_path");
/// ```
pub(crate) fn only(report: &SkillValidationReport) -> &ValidationFinding {
    assert_eq!(report.findings.len(), 1, "expected exactly one finding");
    report
        .findings
        .first()
        .expect("length was asserted to be one")
}
