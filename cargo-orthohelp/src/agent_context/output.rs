//! Bridge mapping from IR structured-output declarations to agent contracts.
//!
//! This is the only place the IR's strict stdout enum meets the wire's
//! tolerant one. Every variant is mapped explicitly so a dropped arm fails a
//! test rather than silently reporting `unknown`, and nothing here infers,
//! sorts, localizes, or populates `output_modes`.
//!
//! The mapping is field-by-field and per command: an absent block stays
//! absent, an explicitly refused capability stays `Some(false)`, and a child
//! never inherits its parent's declaration.

use ortho_config::agent_context::{
    ExitClass as AgentExitClass, OutputContract, StdoutPurpose as AgentStdoutPurpose,
};

use crate::schema::{ExitClass, OutputMetadata, StdoutPurpose};

/// Maps an optional IR output block onto the agent-context contract.
///
/// An absent block yields `None`; the bridge does not invent a default such as
/// `supports_json: false`, because an agent must be able to tell an author's
/// refusal from an author's silence.
pub(super) fn map_output(declared: Option<&OutputMetadata>) -> Option<OutputContract> {
    declared.map(|output| OutputContract {
        supports_json: output.supports_json,
        stdout: output.stdout.map(map_stdout),
        stderr_diagnostics: output.stderr_diagnostics,
        exit_classes: output.exit_classes.iter().map(map_exit_class).collect(),
    })
}

/// Maps a declared stdout purpose, preserving the author's declaration exactly.
const fn map_stdout(value: StdoutPurpose) -> AgentStdoutPurpose {
    match value {
        StdoutPurpose::Data => AgentStdoutPurpose::Data,
        StdoutPurpose::Path => AgentStdoutPurpose::Path,
        StdoutPurpose::Summary => AgentStdoutPurpose::Summary,
        StdoutPurpose::None => AgentStdoutPurpose::None,
    }
}

/// Maps one documented exit class, preserving its order and untouched text.
fn map_exit_class(class: &ExitClass) -> AgentExitClass {
    AgentExitClass {
        name: class.name.clone(),
        meaning: class.meaning.clone(),
    }
}
