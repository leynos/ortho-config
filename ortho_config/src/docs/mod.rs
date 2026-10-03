//! Documentation metadata for `OrthoConfig`.
//!
//! This module defines the IR schema used by `cargo-orthohelp` and the
//! `OrthoConfigDocs` trait implemented by the derive macro.

mod ir;

pub use ir::{
    CliMetadata, ConfigDiscoveryMeta, ConfigFormat, DefaultValue, Deprecation, DocMetadata,
    EnvMetadata, Example, FieldMetadata, FileMetadata, HeadingIds, Link, Note, PathPattern,
    PrecedenceMeta, SectionsMetadata, SourceKind, ValueType, WindowsMetadata,
};

/// Current IR schema version.
pub const ORTHO_DOCS_IR_VERSION: &str = "2.0";

/// Trait implemented for configs that can emit documentation metadata.
pub trait OrthoConfigDocs {
    /// Returns the complete documentation metadata for this config.
    fn get_doc_metadata() -> DocMetadata;

    /// Returns documentation metadata for this config mounted at `command_path`.
    ///
    /// Handwritten implementations may rely on this compatibility fallback when
    /// they do not need path-sensitive generated identifiers.
    #[must_use]
    fn get_doc_metadata_for_path(_command_path: &[String]) -> DocMetadata {
        Self::get_doc_metadata()
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
/// #[ortho_config(prefix = "APP_")]
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
