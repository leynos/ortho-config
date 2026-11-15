//! Plain-data shapes exchanged between the documentation field builders.
//!
//! These carry already-resolved values and token streams between the builder
//! in `super`, the value-type model, and the renderer. They hold no logic, so
//! they live apart from the builder that fills them and from the metadata
//! helpers that consume them.

use proc_macro2::TokenStream;
use syn::Ident;

use crate::derive::build::CliFieldMetadata;
use crate::derive::generate::localization::LocalizationIds;
use crate::derive::parse::{FieldAttrs, SerdeRenameAll};

use super::super::AppName;
use super::value_types::ValueTypeModel;

/// Inputs one struct's documentation pass needs to build its field metadata.
///
/// Assembled by the caller in `docs` and consumed by the builder in `super`.
/// It is `pub(crate)` rather than `pub(super)` because the re-export in
/// `super` keeps one import path for both call sites, and a re-export cannot
/// widen the visibility of what it names.
pub(crate) struct FieldDocArgs<'a> {
    /// Application name used to compose default catalogue identifiers.
    pub app_name: &'a AppName,
    /// Environment-variable prefix for this struct, when one is configured.
    pub prefix: Option<&'a str>,
    /// Struct fields, in declaration order.
    pub fields: &'a [syn::Field],
    /// One parsed attribute set per entry in `fields`.
    pub field_attrs: &'a [FieldAttrs],
    /// Container-level renaming applied when deriving a field's file key.
    pub serde_rename_all: Option<SerdeRenameAll>,
    /// CLI metadata already collected for this struct's fields.
    pub cli_fields: &'a [CliFieldMetadata],
    /// Catalogue identifiers generated for this struct.
    pub localization_ids: &'a LocalizationIds,
    /// Resolved crate path for generated code references.
    pub krate: &'a proc_macro2::TokenStream,
}

/// Everything one field's builders read, gathered before rendering starts.
pub(super) struct FieldContext<'a> {
    /// Field identifier, used as the span for field-local diagnostics.
    pub(super) name: &'a Ident,
    /// Field name as written in the source.
    pub(super) field_name: &'a str,
    /// The field itself, for type and key analysis.
    pub(super) field: &'a syn::Field,
    /// Attributes parsed for this field.
    pub(super) attrs: &'a FieldAttrs,
    /// Resolved value type, absent when the field is skipped or unrecognized.
    pub(super) value_type: Option<&'a ValueTypeModel>,
}

/// Borrows one field's resolved identity expressions before emission.
pub(super) struct FieldIdentity<'a> {
    /// Field name to emit as the metadata's `name` entry.
    pub(super) field_name: &'a str,
    /// Catalogue key for the field's one-line help.
    pub(super) help_id: TokenStream,
    /// Catalogue key for the field's long help.
    pub(super) long_help_id: TokenStream,
}

/// Pre-computed expressions for a field's value type and requiredness.
pub(super) struct ValueContext {
    /// Tokens building the `ValueType` of the field.
    pub(super) value_tokens: TokenStream,
    /// Whether the field must be supplied.
    pub(super) required: bool,
}

/// One field's rendered CLI, environment, and file metadata expressions.
pub(super) struct IoTokens {
    /// Expression building the optional `CliMetadata`.
    pub(super) cli: TokenStream,
    /// Expression building the `EnvMetadata`.
    pub(super) env: TokenStream,
    /// Expression building the `FileMetadata`.
    pub(super) file: TokenStream,
}

/// Rendered vectors and options for a field's non-identity metadata.
///
/// Kept as separate vectors rather than one merged stream so the renderer can
/// place each generated field independently; the order inside each is the
/// declaration order the author wrote.
pub(super) struct MetaParts {
    /// Expression for the field's default value, when one is declared.
    pub(super) default_tokens: TokenStream,
    /// Expression for the field's deprecation notice, when one is declared.
    pub(super) deprecated_tokens: TokenStream,
    /// One expression per documented example.
    pub(super) examples: Vec<TokenStream>,
    /// One expression per documented link.
    pub(super) links: Vec<TokenStream>,
    /// One expression per documented note.
    pub(super) notes: Vec<TokenStream>,
}

/// Bundles all components needed to render field metadata tokens.
pub(super) struct FieldMetadataComponents {
    /// Identity expressions built from the field's catalogue keys.
    pub(super) identity: FieldIdentityTokens,
    /// Value type and requiredness expressions.
    pub(super) value_context: ValueContext,
    /// CLI, environment, and file metadata expressions.
    pub(super) io_tokens: IoTokens,
    /// Defaults, deprecations, examples, links, and notes.
    pub(super) meta_parts: MetaParts,
}

/// Pre-computed expressions for field identity.
pub(super) struct FieldIdentityTokens {
    /// Field name literal emitted as the metadata's `name`.
    pub(super) field_name: syn::LitStr,
    /// Catalogue key for the field's one-line help.
    pub(super) help_id: TokenStream,
    /// Catalogue key for the field's long help.
    pub(super) long_help: TokenStream,
}
