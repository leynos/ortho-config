//! Compile-fail test for the skill-validation API's `#[must_use]` items.
//!
//! A report is the only product of a validation run, so discarding a query
//! throws that product away silently. The surface adapter and the glossary
//! lookups answer questions whose answer is the whole reason for the call.
//! `unused_must_use` must reject each of them.

#![deny(unused_must_use)]

use cargo_orthohelp::policy::vocabulary::global_options::concept_for_long_name;
use cargo_orthohelp::policy::PolicyMode;
use cargo_orthohelp::skill_validation::{
    CommandContract, EffectiveCommandSurface, ManifestClaims, ResolvedAgentPolicy,
    SkillValidationInput, validate,
};
use ortho_config::{AgentContext, SkillCommandRef};

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
                path,
                flags: Vec::new(),
            }],
        }],
        examples: Vec::new(),
    };
    let policy = ResolvedAgentPolicy {
        mode: PolicyMode::Off,
        exceptions: Vec::new(),
    };
    let report = validate(&input, &surface, &policy).expect("surface is unambiguous");
    report.is_complete();
    report.has_structural_findings();
    report.has_policy_findings();
    EffectiveCommandSurface::from_agent_context(&AgentContext::new("tool"), &[]);
    concept_for_long_name("--color");
}