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
pub const ORTHO_DOCS_IR_VERSION: &str = "1.1";

/// Trait implemented for configs that can emit documentation metadata.
pub trait OrthoConfigDocs {
    /// Returns the complete documentation metadata for this config.
    fn get_doc_metadata() -> DocMetadata;
}
