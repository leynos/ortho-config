//! Assemble generated field metadata from resolved identities and source tokens.
//!
//! This module owns the final token layout so field validation and value
//! resolution remain separate from the runtime metadata initializer shape.

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::derive::parse::FieldAttrs;

use super::super::{example_tokens, link_tokens, note_tokens};
use super::tokens::{default_tokens, deprecated_tokens};
use super::value_types::ValueTypeModel;

/// Source information shared by the independent field metadata renderers.
pub(super) struct FieldContext<'a> {
    /// Identifier used to attach source spans to validation diagnostics.
    pub(super) name: &'a Ident,
    /// Original Rust field spelling, also used to find its CLI record.
    pub(super) field_name: &'a str,
    /// Full syntax node needed for type and Serde-key resolution.
    pub(super) field: &'a syn::Field,
    /// Parsed per-field behaviour and documentation overrides.
    pub(super) attrs: &'a FieldAttrs,
    /// Resolved type description, absent when neither override nor inference fits.
    pub(super) value_type: Option<&'a ValueTypeModel>,
}

/// Source strings that identify a field in generated user-facing metadata.
pub(super) struct FieldIdentity<'a> {
    /// Rust field name retained as the stable field identity.
    pub(super) field_name: &'a str,
    /// Resolved short-help catalogue key.
    pub(super) help_id: &'a str,
    /// Resolved long-help catalogue key.
    pub(super) long_help_id: &'a str,
}

/// Value description and requiredness emitted together in a field record.
pub(super) struct ValueContext {
    /// Tokens for `Some(ValueType)` or `None` when the type is not describable.
    pub(super) value_tokens: TokenStream,
    /// Whether configuration must supply a value after defaults are considered.
    pub(super) required: bool,
}

/// Generated token fragments for the field's supported input sources.
pub(super) struct IoTokens {
    /// Optional CLI metadata; skipped fields emit `None`.
    pub(super) cli: TokenStream,
    /// Environment-variable metadata, always present for a documented field.
    pub(super) env: TokenStream,
    /// File key metadata, always present for a documented field.
    pub(super) file: TokenStream,
}

/// Tokenized defaults and supplementary annotations attached to a field.
pub(super) struct MetaParts {
    /// Default value metadata or `None` when no default is declared.
    default_tokens: TokenStream,
    /// Deprecation metadata or `None` when the field is current.
    deprecated_tokens: TokenStream,
    /// Ordered examples from the parsed documentation attributes.
    examples: Vec<TokenStream>,
    /// Ordered reference links from the parsed documentation attributes.
    links: Vec<TokenStream>,
    /// Ordered free-form notes from the parsed documentation attributes.
    notes: Vec<TokenStream>,
}

/// Converts parsed field annotations into tokens used in the metadata record.
pub(super) fn render_meta_parts(attrs: &FieldAttrs, krate: &proc_macro2::TokenStream) -> MetaParts {
    MetaParts {
        default_tokens: default_tokens(attrs, krate),
        deprecated_tokens: deprecated_tokens(attrs, krate),
        examples: example_tokens(&attrs.doc.examples, krate),
        links: link_tokens(&attrs.doc.links, krate),
        notes: note_tokens(&attrs.doc.notes, krate),
    }
}

/// Complete intermediate record passed to the final metadata renderer.
pub(super) struct FieldMetadataComponents {
    /// Pre-tokenized field name and help catalogue identifiers.
    pub(super) identity: FieldIdentityTokens,
    /// Value type and requiredness rendered as a single metadata section.
    pub(super) value_context: ValueContext,
    /// Input-source metadata for CLI, environment, and file configuration.
    pub(super) io_tokens: IoTokens,
    /// Defaults and supplementary user-authored annotations.
    pub(super) meta_parts: MetaParts,
}

/// Pre-computed string literals for identity fields in generated metadata.
pub(super) struct FieldIdentityTokens {
    /// Literal preserving the source field's Rust spelling.
    field_name: syn::LitStr,
    /// Literal for the resolved short-help key.
    help_id: syn::LitStr,
    /// Literal for the resolved long-help key.
    long_help: syn::LitStr,
}

impl FieldIdentity<'_> {
    /// Converts resolved source strings to literals at the macro call site.
    pub(super) fn into_tokens(self) -> FieldIdentityTokens {
        FieldIdentityTokens {
            field_name: syn::LitStr::new(self.field_name, proc_macro2::Span::call_site()),
            help_id: syn::LitStr::new(self.help_id, proc_macro2::Span::call_site()),
            long_help: syn::LitStr::new(self.long_help_id, proc_macro2::Span::call_site()),
        }
    }
}

/// Emits the public `FieldMetadata` initializer consumed by generated code.
pub(super) fn render_field_metadata(
    components: FieldMetadataComponents,
    krate: &proc_macro2::TokenStream,
) -> TokenStream {
    let FieldMetadataComponents {
        identity,
        value_context,
        io_tokens,
        meta_parts,
    } = components;

    let identity_tokens = render_identity_tokens(identity);
    let value_tokens = render_value_tokens(value_context);
    let io = render_io_block(io_tokens);
    let meta = render_meta_block(meta_parts);

    quote! {
        #krate::docs::FieldMetadata {
            #identity_tokens
            #value_tokens
            #io
            #meta
        }
    }
}

/// Emits the field name and help identifiers with owned `String` values.
fn render_identity_tokens(identity: FieldIdentityTokens) -> TokenStream {
    let field_name = identity.field_name;
    let help_id = identity.help_id;
    let long_help = identity.long_help;
    quote! {
        name: String::from(#field_name),
        help_id: String::from(#help_id),
        long_help_id: Some(String::from(#long_help)),
    }
}

/// Emits the optional type description and requiredness flag.
fn render_value_tokens(value: ValueContext) -> TokenStream {
    let value_tokens = value.value_tokens;
    let required = value.required;
    quote! {
        value: #value_tokens,
        required: #required,
    }
}

/// Emits input-source metadata while preserving the optional CLI distinction.
fn render_io_block(io: IoTokens) -> TokenStream {
    let cli = io.cli;
    let env = io.env;
    let file = io.file;
    quote! {
        cli: #cli,
        env: Some(#env),
        file: Some(#file),
    }
}

/// Emits defaults, deprecation state, and all supplementary annotations.
fn render_meta_block(meta: MetaParts) -> TokenStream {
    let default = meta.default_tokens;
    let deprecated = meta.deprecated_tokens;
    let examples = render_vec_field("examples", &meta.examples);
    let links = render_vec_field("links", &meta.links);
    let notes = render_vec_field("notes", &meta.notes);
    quote! {
        default: #default,
        deprecated: #deprecated,
        #examples
        #links
        #notes
    }
}

/// Emits a named vector initializer while retaining the input item order.
fn render_vec_field(field_name: &str, items: &[TokenStream]) -> TokenStream {
    let ident = syn::Ident::new(field_name, proc_macro2::Span::call_site());
    quote! { #ident: vec![ #( #items ),* ], }
}
