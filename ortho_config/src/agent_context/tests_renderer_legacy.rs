//! Additive-compatibility coverage for the renderer contract.

use super::*;
use rstest::rstest;

/// The frozen pre-renderer document still parses and gains only defaults.
///
/// This is the additive-compatibility guarantee for the `renderer` field: an
/// older producer's JSON keeps working, and the reader supplies the documented
/// legacy support values rather than inventing presentation policy.
#[rstest]
fn frozen_prerenderer_document_applies_legacy_renderer_defaults() {
    let context: AgentContext =
        serde_json::from_str(FROZEN_PRERENDERER_WIRE_JSON).expect("parse frozen pre-renderer JSON");
    let command = context
        .commands
        .first()
        .expect("frozen document should contain one command");

    assert_eq!(command.renderer, RendererContract::default());
    assert!(
        command.renderer.human.supported,
        "human support defaults true"
    );
    assert!(
        !command.renderer.machine.supported,
        "machine support defaults false"
    );
    assert!(
        command.renderer.human.tty.is_none() && command.renderer.machine.tty.is_none(),
        "an old document must not acquire a presentation claim it never made"
    );
    assert!(
        command.renderer.human.fields.is_empty() && command.renderer.machine.fields.is_empty(),
        "an old document must not acquire a field-localization claim"
    );
}
