//! Agent-native behaviour declarations carried in the documentation IR.
//!
//! These types let an author state how a command behaves instead of leaving
//! consumers to infer it from a command's name, verbs, or flags. Every field is
//! optional, and the IR deliberately has no `Unknown` variant: an absent
//! declaration is expressed by the corresponding `Option` being `None`, and
//! only the agent-context layer models the `unknown` wire state.

use serde::{Deserialize, Serialize};

/// Declared execution behaviour for a command (agent-native metadata).
///
/// Every field is optional: a present [`BehaviourMetadata`] with all fields
/// `None` is equivalent to no declaration at all. Undeclared aspects stay
/// undeclared; the agent-context layer maps them to its `unknown` state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviourMetadata {
    /// Interaction declaration; `None` means undeclared.
    #[serde(default)]
    pub interaction: Option<InteractionKind>,
    /// Mutation boundary declaration; `None` means undeclared.
    #[serde(default)]
    pub mutation: Option<MutationKind>,
    /// Confirmation/prompt bypass flag, for example `--force`.
    #[serde(default)]
    pub bypass: Option<String>,
    /// Dry-run flag name, for example `--dry-run`; `None` means undeclared.
    #[serde(default)]
    pub dry_run: Option<String>,
}

/// Declared interaction behaviour for a command in the documentation IR.
///
/// The IR has no `Unknown` variant on purpose: absence of a declaration is
/// represented by [`BehaviourMetadata::interaction`] being `None`, and only
/// the agent-context layer models the `unknown` wire state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    /// Command does not prompt and can run unattended.
    NonInteractive,
    /// Command may require operator interaction.
    Interactive,
}

/// Declared mutation boundary for a command in the documentation IR.
///
/// Absence of a declaration is represented by
/// [`BehaviourMetadata::mutation`] being `None`; see [`InteractionKind`] for
/// why there is no `Unknown` variant at this layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationKind {
    /// Read-only command.
    ReadOnly,
    /// Command may write local or remote state.
    Write,
    /// Command may delete local or remote state.
    Delete,
    /// Command submits asynchronous work.
    Submit,
}
