//! Documentation metadata for `OrthoConfig`.
//!
//! This module defines the IR schema used by `cargo-orthohelp` and the
//! `OrthoConfigDocs` trait implemented by the derive macro.

mod ir;

mod profile_redaction;

#[cfg(test)]
mod proptests;

pub use ir::{
    AccessibilityPolicy, BehaviourMetadata, CliMetadata, ClosedStdinPolicy, ConfigDiscoveryMeta,
    ConfigFormat, DefaultValue, Deprecation, DocMetadata, DocProfilesMeta, EnvMetadata, Example,
    ExitClass, FieldMetadata, FileMetadata, HeadingIds, InheritancePolicy, InteractionKind, Link,
    MutationKind, Note, OutputMetadata, PathPattern, PlainPolicy, PositionalMetadata,
    PrecedenceMeta, RenderWidth, RendererField, RendererFieldLocalization, RendererMetadata,
    RendererRecord, SectionsMetadata, SourceKind, StdoutPurpose, StreamContent, TogglePolicy,
    TtyPolicy, ValueType, WindowsMetadata,
};
pub use profile_redaction::ProfileFieldExposure;

/// Current IR schema version.
pub const ORTHO_DOCS_IR_VERSION: &str = "2.1";

/// Trait implemented for configs that can emit documentation metadata.
pub trait OrthoConfigDocs {
    /// Returns the complete documentation metadata for this config.
    fn get_doc_metadata() -> DocMetadata;

    /// Returns documentation metadata for this config mounted at `command_path`.
    ///
    /// A generated implementation also treats the last path segment as the
    /// subcommand name and projects environment-variable and file-key metadata
    /// into the `<PREFIX>_CMDS_<NAME>_` and `cmds.<name>` namespaces the
    /// subcommand merge reads. An empty path is equivalent to
    /// [`Self::get_doc_metadata`].
    ///
    /// Handwritten implementations may rely on this compatibility fallback when
    /// they do not need path-sensitive generated identifiers.
    #[must_use]
    fn get_doc_metadata_for_path(_command_path: &[String]) -> DocMetadata {
        Self::get_doc_metadata()
    }

    /// Returns metadata for this config mounted as the subcommand `name`.
    ///
    /// `parent_path` carries the enclosing command path and is empty when the
    /// subcommand sits at the root of the tree. A generated implementation
    /// uses the struct's declared `localization_base` for default identifiers
    /// when no parent is supplied, so `about_id` and the field help identifiers
    /// agree with `ABOUT_ID` and `ARG_IDS`. With a parent path, the identifiers
    /// follow `parent_path` plus `name`, agreeing with the path
    /// `LocalizeCmd::with_base(..).localize(..)` resolves.
    ///
    /// Environment-variable and file-key metadata are projected into the
    /// `<PREFIX>_CMDS_<NAME>_` and `cmds.<name>` namespaces the subcommand
    /// merge reads. A handwritten implementation reaches
    /// `get_doc_metadata_for_path(parent_path plus name)`, so one that
    /// overrides that method receives the root-level subcommand path; one
    /// without a path override still falls back to the existing metadata with
    /// only the application name replaced.
    #[must_use]
    fn get_doc_metadata_for_subcommand(parent_path: &[String], name: &str) -> DocMetadata {
        let mut command_path = parent_path.to_vec();
        command_path.push(name.to_owned());
        let mut metadata = Self::get_doc_metadata_for_path(&command_path);
        name.clone_into(&mut metadata.app_name);
        metadata
    }
}

/// Trait implemented for `clap::Subcommand` enums that emit per-variant
/// documentation metadata.
///
/// Each returned [`DocMetadata`] entry describes one subcommand variant. The
/// generated implementation preserves enum declaration order so generated
/// documentation remains deterministic.
///
/// # Examples
///
/// ```rust,ignore
/// use clap::{Parser, Subcommand};
/// use ortho_config::{OrthoConfig, OrthoConfigSubcommandDocs};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Parser, OrthoConfig)]
/// #[ortho_config(prefix = "APP_")]
/// struct Cli {
///     #[command(subcommand)]
///     command: Commands,
/// }
///
/// #[derive(Subcommand, OrthoConfigSubcommandDocs)]
/// enum Commands {
///     Run(RunArgs),
/// }
///
/// #[derive(Parser, Serialize, Deserialize, Default, OrthoConfig)]
/// #[ortho_config(
///     prefix = "APP_",
///     behaviour(interaction = "interactive", mutation = "read_only")
/// )]
/// struct RunArgs {
///     #[arg(long)]
///     name: String,
/// }
/// ```
pub trait OrthoConfigSubcommandDocs {
    /// Returns one [`DocMetadata`] per subcommand variant.
    fn get_subcommand_doc_metadata() -> Vec<DocMetadata>;

    /// Returns one [`DocMetadata`] per subcommand variant mounted below
    /// `parent_path`.
    ///
    /// Handwritten implementations may rely on this compatibility fallback when
    /// they do not need path-sensitive generated identifiers.
    #[must_use]
    fn get_subcommand_doc_metadata_for_path(_parent_path: &[String]) -> Vec<DocMetadata> {
        Self::get_subcommand_doc_metadata()
    }
}
