//! Bridge tests for declared and undeclared renderer metadata (E1, C1/C2).
//!
//! The bridge is the only place the IR's strict enums meet the wire's tolerant
//! ones, so every accepted variant gets an explicit case: a dropped or swapped
//! `match` arm must fail here rather than silently degrading to `unknown`.
//!
//! Two cases exist specifically to keep the default tests honest. An absent
//! declaration and an explicit `supported = false` both look "not supported" if
//! you only read the machine record, so both are asserted independently.

use ortho_config::{RendererContract, RendererRecord, StreamContent, TogglePolicy, TtyPolicy};
use rstest::rstest;

use super::support::*;
use crate::agent_context::bridge_ir_to_agent_context;
use crate::schema::{
    RendererField as IrField, RendererFieldLocalization as IrFieldLocalization, RendererMetadata,
    RendererRecord as IrRecord, StreamContent as IrStream, TogglePolicy as IrToggle,
    TtyPolicy as IrTty,
};

/// Returns the renderer of the single command in a one-command tree.
pub(super) fn bridge_single(renderer: Option<RendererMetadata>) -> RendererContract {
    let mut metadata = doc(DocSpec::root_without_fields());
    metadata.renderer = renderer;
    let context = bridge_ir_to_agent_context(&metadata, "demo_pkg", None);
    context
        .commands
        .into_iter()
        .next()
        .expect("a root command is always emitted")
        .renderer
}

/// No declaration yields exactly the documented legacy support defaults.
///
/// This is the C2 default case. It also pins that undeclared policy stays
/// absent, so a consumer cannot mistake "no claim" for an affirmative promise.
#[test]
fn absent_declaration_yields_only_legacy_support_defaults() {
    let renderer = bridge_single(None);

    assert_eq!(
        renderer,
        RendererContract::default(),
        "an absent block must project to the legacy defaults"
    );
    assert!(renderer.human.supported, "human support defaults true");
    assert!(
        !renderer.machine.supported,
        "machine support defaults false"
    );
    assert_eq!(
        renderer.human,
        RendererRecord {
            supported: true,
            ..RendererRecord::default()
        },
        "no policy may be invented for the human record"
    );
}

/// An empty renderer object behaves exactly like an absent block.
#[test]
fn empty_renderer_block_yields_legacy_support_defaults() {
    let renderer = bridge_single(Some(RendererMetadata::default()));

    assert_eq!(renderer, RendererContract::default());
}

/// A record that omits `supported` keeps its role-specific default.
///
/// Declaring only a policy must not silently flip support, which is why the
/// bridge threads `role_default` instead of relying on `bool::default()`.
#[test]
fn policy_only_record_keeps_role_specific_support_default() {
    let mut metadata = doc(DocSpec::root_without_fields());
    metadata.renderer = Some(RendererMetadata {
        human: Some(IrRecord {
            colour: Some(IrToggle::Auto),
            ..IrRecord::default()
        }),
        machine: Some(IrRecord {
            colour: Some(IrToggle::Never),
            ..IrRecord::default()
        }),
    });

    let renderer = bridge_single(metadata.renderer);

    assert!(renderer.human.supported, "human support stays true");
    assert!(!renderer.machine.supported, "machine support stays false");
    assert_eq!(renderer.human.colour, Some(TogglePolicy::Auto));
    assert_eq!(renderer.machine.colour, Some(TogglePolicy::Never));
}

/// All four support combinations survive the projection unchanged (C1).
///
/// The `explicit_false_human` row is the witness that defaulting cannot pass
/// vacuously: a bridge that ignored the declared leaf would report `true`.
#[rstest]
#[case::both_declared_true(Some(true), Some(true), true, true)]
#[case::explicit_false_human(Some(false), Some(false), false, false)]
#[case::human_only(Some(true), None, true, false)]
#[case::machine_only(None, Some(true), true, true)]
fn support_combinations_survive_projection(
    #[case] human_supported: Option<bool>,
    #[case] machine_supported: Option<bool>,
    #[case] expected_human: bool,
    #[case] expected_machine: bool,
) {
    let mut metadata = doc(DocSpec::root_without_fields());
    metadata.renderer = Some(RendererMetadata {
        human: human_supported.map(|supported| IrRecord {
            supported: Some(supported),
            ..IrRecord::default()
        }),
        machine: machine_supported.map(|supported| IrRecord {
            supported: Some(supported),
            ..IrRecord::default()
        }),
    });

    let renderer = bridge_single(metadata.renderer);

    assert_eq!(renderer.human.supported, expected_human);
    assert_eq!(renderer.machine.supported, expected_machine);
}

/// Field declarations keep their source order rather than being sorted.
///
/// The bridge must not reorder entries: the plan forbids sorting, and a
/// consumer comparing two documents relies on declaration order.
#[test]
fn field_declarations_preserve_declaration_order() {
    let renderer = bridge_single(Some(RendererMetadata {
        human: Some(IrRecord {
            fields: vec![
                IrField {
                    id: "zeta_message".to_owned(),
                    localization: IrFieldLocalization::Localized,
                },
                IrField {
                    id: "alpha_code".to_owned(),
                    localization: IrFieldLocalization::NonLocalized,
                },
            ],
            ..IrRecord::default()
        }),
        machine: None,
    }));

    let ids: Vec<_> = renderer
        .human
        .fields
        .iter()
        .map(|f| f.id.as_str())
        .collect();
    assert_eq!(ids, ["zeta_message", "alpha_code"]);
}

/// Changing one renderer leaves the other untouched (C1 independence).
#[test]
fn renderer_records_are_independent() {
    let renderer = bridge_single(Some(RendererMetadata {
        human: Some(IrRecord {
            supported: Some(true),
            colour: Some(IrToggle::Auto),
            tty: Some(IrTty::Adapts),
            ..IrRecord::default()
        }),
        machine: Some(IrRecord::default()),
    }));

    assert_eq!(renderer.human.colour, Some(TogglePolicy::Auto));
    assert_eq!(renderer.human.tty, Some(TtyPolicy::Adapts));
    assert_eq!(
        renderer.machine,
        RendererRecord::default(),
        "the machine record must not inherit the human policies"
    );
}

/// A child with no declaration gets its own defaults, never the parent's (C2).
#[test]
fn child_commands_do_not_inherit_renderer_declarations() {
    let mut root = doc(DocSpec {
        subcommands: vec![doc(DocSpec::child("status", "cmd.status"))],
        ..DocSpec::root_without_fields()
    });
    root.renderer = Some(RendererMetadata {
        human: Some(IrRecord {
            supported: Some(true),
            colour: Some(IrToggle::Always),
            tty: Some(IrTty::Requires),
            ..IrRecord::default()
        }),
        machine: Some(IrRecord {
            supported: Some(true),
            stdout: Some(IrStream::Structured),
            ..IrRecord::default()
        }),
    });

    let context = bridge_ir_to_agent_context(&root, "demo_pkg", None);
    let child = context
        .commands
        .iter()
        .find(|command| command.path.last().map(String::as_str) == Some("status"))
        .expect("the child command should be present");

    assert_eq!(
        child.renderer,
        RendererContract::default(),
        "an undeclared child must carry only the legacy support defaults"
    );
}

/// Sibling commands keep their own declarations (C1 independence).
#[test]
fn sibling_commands_keep_their_own_declarations() {
    let mut human_sibling = doc(DocSpec::child("render", "cmd.render"));
    human_sibling.renderer = Some(RendererMetadata {
        human: Some(IrRecord {
            colour: Some(IrToggle::Always),
            ..IrRecord::default()
        }),
        machine: None,
    });
    let mut machine_sibling = doc(DocSpec::child("emit", "cmd.emit"));
    machine_sibling.renderer = Some(RendererMetadata {
        human: None,
        machine: Some(IrRecord {
            supported: Some(true),
            stdout: Some(IrStream::Structured),
            ..IrRecord::default()
        }),
    });
    let root = doc(DocSpec {
        subcommands: vec![human_sibling, machine_sibling],
        ..DocSpec::root_without_fields()
    });

    let context = bridge_ir_to_agent_context(&root, "demo_pkg", None);
    let find = |name: &str| {
        &context
            .commands
            .iter()
            .find(|command| command.path.last().map(String::as_str) == Some(name))
            .expect("the sibling command should be present")
            .renderer
    };
    let render_sibling = find("render");
    let emit_sibling = find("emit");

    assert_eq!(render_sibling.human.colour, Some(TogglePolicy::Always));
    assert_eq!(
        render_sibling.machine,
        RendererRecord::default(),
        "the human-only sibling must not acquire machine policy"
    );
    assert_eq!(emit_sibling.machine.stdout, Some(StreamContent::Structured));
    assert_eq!(
        emit_sibling.human,
        RendererRecord {
            supported: true,
            ..RendererRecord::default()
        },
        "the machine-only sibling keeps only the legacy human support default"
    );
}
