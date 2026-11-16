//! Compile-pass test for the public skill-validation API.
//!
//! A consumer outside this crate builds a surface and an input from struct
//! literals, resolves a policy, and reads one report. This fixture keeps that
//! path compiling and pins the code a consumer matches on, so a field added
//! for internal convenience cannot quietly become a required consumer field.

use cargo_orthohelp::policy::PolicyMode;
use cargo_orthohelp::skill_validation::{
    CommandContract, EffectiveCommandSurface, ManifestClaims, ResolvedAgentPolicy,
    SkillValidationInput, codes, validate,
};
use ortho_config::SkillCommandRef;

fn main() {
    let path = vec!["tool".to_owned(), "records".to_owned(), "get".to_owned()];
    let surface = EffectiveCommandSurface {
        commands: vec![CommandContract {
            path: path.clone(),
            options: Vec::new(),
            canonical_verb: Some("get".to_owned()),
        }],
    };
    let input = SkillValidationInput {
        manifests: vec![ManifestClaims {
            id: "operation-skill".to_owned(),
            commands: vec![SkillCommandRef {
                path: path.clone(),
                flags: vec!["json".to_owned()],
            }],
        }],
        examples: Vec::new(),
    };
    let policy = ResolvedAgentPolicy {
        mode: PolicyMode::Deny,
        exceptions: Vec::new(),
    };

    let report = validate(&input, &surface, &policy).expect("surface is unambiguous");

    let finding = report
        .findings
        .first()
        .expect("a flag the command does not accept is reported");
    assert_eq!(finding.code, codes::UNKNOWN_FLAG);
    assert_eq!(finding.command_path, path);
    assert_eq!(finding.option.as_deref(), Some("json"));
    assert!(report.has_structural_findings());
    assert!(!report.has_policy_findings());
}