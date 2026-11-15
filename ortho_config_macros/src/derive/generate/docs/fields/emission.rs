//! Token emission for resolved field documentation records.
//!
//! Field resolution and duplicate detection remain in the parent builder.
//! This module only assembles its resolved components into the public IR
//! initializer, preserving annotation order and caller-supplied crate paths.

use proc_macro2::TokenStream;
use quote::quote;

use super::shapes::{
    FieldIdentityTokens, FieldMetadataComponents, IoTokens, MetaParts, ValueContext,
};

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
        help_id: #help_id,
        long_help_id: Some(#long_help),
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
