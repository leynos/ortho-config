//! Documentation IR schema used by `cargo-orthohelp`.
//!
//! The schema is owned by `ortho_config::docs` (ADR-0003). This module
//! re-exports the owner's types instead of mirroring them, so the tool and the
//! derive macro can never drift into two subtly different IRs, and one
//! redaction projection covers both. Every type stays re-exported from this
//! module, so `crate::schema::DocMetadata` and friends remain valid paths for
//! the rest of the tool.

pub use ortho_config::docs::{
    AccessibilityPolicy, BehaviourMetadata, CliMetadata, ClosedStdinPolicy, ConfigDiscoveryMeta,
    ConfigFormat, DefaultValue, Deprecation, DocMetadata, DocProfilesMeta, EnvMetadata, Example,
    ExitClass, FieldMetadata, FileMetadata, HeadingIds, InheritancePolicy, InteractionKind, Link,
    MutationKind, Note, ORTHO_DOCS_IR_VERSION, OutputMetadata, PathPattern, PlainPolicy,
    PrecedenceMeta, ProfileFieldExposure, RenderWidth, RendererField, RendererFieldLocalization,
    RendererMetadata, RendererRecord, SectionsMetadata, SourceKind, StdoutPurpose, StreamContent,
    TogglePolicy, TtyPolicy, ValueType, WindowsMetadata,
};

#[cfg(test)]
mod tests;
