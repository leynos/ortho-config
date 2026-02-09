//! Shared types for MAML rendering.

use crate::ir::LocalizedDocMetadata;

/// A command entry to render in MAML output.
#[derive(Debug, Clone)]
pub struct CommandSpec<'a> {
    /// Name of the `PowerShell` command.
    pub name: String,
    /// Localized metadata for the command.
    pub metadata: &'a LocalizedDocMetadata,
}

/// Options for MAML generation.
#[derive(Debug, Clone, Copy)]
pub struct MamlOptions {
    /// Include `CommonParameters` in the help output.
    pub should_include_common_parameters: bool,
}
