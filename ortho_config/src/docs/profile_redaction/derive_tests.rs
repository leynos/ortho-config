//! Deep-chain regressions for the derive's projection boundaries.
//!
//! Each derived `get_doc_metadata_for_path` is a public conversion boundary,
//! and a parent composes its children by calling theirs. The projection is
//! therefore applied once per boundary: never zero times, and never once per
//! node in addition to the boundary that owns it. Both halves matter. Removing
//! a boundary's projection discloses the leaf sentinel, and projecting a
//! composed subtree a second time inflates the traversal count.
//!
//! The fixtures live inside the crate so `#[cfg(test)]` instrumentation in
//! [`super::traversal_probe`] is active; an integration test compiles the
//! library without it. `crate` is named as the derive's crate path because a
//! crate cannot refer to itself as `ortho_config` from the inside.

use super::traversal_probe;
use crate::docs::OrthoConfigDocs;

/// Confidential literal that no projection can fabricate.
const DEEP_SENTINEL: &str = "DEEP-CHAIN-SECRET-SENTINEL";

/// Innermost command, the only node carrying a protected field.
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    clap::Parser,
    crate::OrthoConfig,
)]
#[ortho_config(crate = "crate", prefix = "DEEP_")]
struct LeafArgs {
    /// Confidential value.
    #[arg(long, default_value = DEEP_SENTINEL)]
    #[ortho_config(profile_field = "secret", default = String::from(DEEP_SENTINEL))]
    token: String,
}

/// Subcommand selector for the innermost level.
#[derive(Debug, Clone, PartialEq, clap::Subcommand, crate::OrthoConfigSubcommandDocs)]
#[ortho_config(crate = "crate")]
enum LeafCommand {
    /// Run the leaf command.
    Run(LeafArgs),
}

impl Default for LeafCommand {
    fn default() -> Self {
        Self::Run(LeafArgs::default())
    }
}

/// Middle command: selects the innermost level and declares no fields.
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    clap::Parser,
    crate::OrthoConfig,
)]
#[ortho_config(crate = "crate", prefix = "DEEP_")]
struct MiddleArgs {
    /// Selected innermost command.
    #[serde(skip)]
    #[command(subcommand)]
    command: LeafCommand,
}

/// Subcommand selector for the middle level.
#[derive(Debug, Clone, PartialEq, clap::Subcommand, crate::OrthoConfigSubcommandDocs)]
#[ortho_config(crate = "crate")]
enum MiddleCommand {
    /// Run the middle command.
    Run(MiddleArgs),
}

impl Default for MiddleCommand {
    fn default() -> Self {
        Self::Run(MiddleArgs::default())
    }
}

/// Root command: selects the middle level and declares no fields.
#[derive(
    Debug,
    Default,
    Clone,
    PartialEq,
    serde::Deserialize,
    serde::Serialize,
    clap::Parser,
    crate::OrthoConfig,
)]
#[ortho_config(crate = "crate", prefix = "DEEP_")]
struct RootArgs {
    /// Selected middle command.
    #[serde(skip)]
    #[command(subcommand)]
    command: MiddleCommand,
}

/// Navigates to the protected leaf field.
fn leaf_field(metadata: &crate::docs::DocMetadata) -> &crate::docs::FieldMetadata {
    metadata
        .subcommands
        .first()
        .and_then(|middle| middle.subcommands.first())
        .and_then(|leaf| leaf.fields.first())
        .expect("three-level chain has a leaf field")
}

#[test]
fn derived_deep_chain_is_projected() {
    let metadata = RootArgs::get_doc_metadata();

    let leaf = leaf_field(&metadata);
    assert!(
        leaf.default.is_none(),
        "the innermost boundary must project the protected default"
    );
}

#[test]
fn derived_deep_chain_projects_once_per_boundary() {
    traversal_probe::reset();
    let metadata = RootArgs::get_doc_metadata();
    let visits = traversal_probe::visits();

    // Three boundaries, each projecting its own subtree of 3, 2 and 1 nodes,
    // with one decision pass and one projection pass per node: 2 * (3 + 2 + 1).
    assert_eq!(
        visits, 12,
        "each derived boundary projects its own subtree exactly once"
    );
    assert!(
        leaf_field(&metadata).default.is_none(),
        "the counted run must still project the leaf"
    );
}
