//! Regression for the composed production path: IR, bridge, surface, finding.
//!
//! `tests_surface` builds an `AgentContext` by hand, which pins the surface
//! adapter but not the bridge's own classification. A surface built from a real
//! `AgentContext` could therefore never be shown to produce
//! `skill.policy.noncanonical_verb` for the `info`/`ls` aliases the rule exists
//! to catch, because `canonical_verb_for` returns `None` outside the canonical
//! set. This test drives the real bridge, so a change to that classification is
//! caught here rather than only in the adapter.

use crate::agent_context::bridge_ir_to_agent_context;
use crate::agent_context::tests::support::{DocSpec, doc};

use super::input::EffectiveCommandSurface;
use super::report::codes;
use super::support::{codes_of, input, manifest, policy, reference};
use super::validate;
use crate::policy::PolicyMode;

#[test]
fn an_alias_verb_from_the_bridged_context_is_advised_from_its_spelling() {
    let metadata = doc(DocSpec {
        app_name: "tool",
        bin_name: Some("tool"),
        about_id: "root.about",
        fields: Vec::new(),
        subcommands: vec![doc(DocSpec::child("info", "cmd.info"))],
        behaviour: None,
    });

    let context = bridge_ir_to_agent_context(&metadata, "tool", None).expect("bridge succeeds");
    let info_commands: Vec<_> = context
        .commands
        .iter()
        .filter(|command| command.path == ["tool".to_owned(), "info".to_owned()])
        .collect();

    assert_eq!(
        info_commands.len(),
        1,
        "the bridge emits one agent command per IR node"
    );
    assert_eq!(
        info_commands
            .first()
            .map(|command| command.canonical_verb.clone()),
        Some(None),
        "the bridge classifies only canonical segments, so `info` carries no verb"
    );

    let surface = EffectiveCommandSurface::from_agent_context(&context, &[]);
    let claims = input(
        vec![manifest(
            "operation-skill",
            vec![reference(&["tool", "info"], &[])],
        )],
        Vec::new(),
    );

    let report = validate(&claims, &surface, &policy(PolicyMode::Deny))
        .expect("the bridge emits an unambiguous surface");

    assert_eq!(
        codes_of(&report),
        vec![codes::POLICY_NONCANONICAL_VERB],
        "an alias the bridge leaves unclassified must still be advised"
    );
}
