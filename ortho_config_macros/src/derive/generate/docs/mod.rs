//! Documentation IR generation for `OrthoConfig`.
//!
//! This module emits `OrthoConfigDocs` implementations that return the
//! `DocMetadata` IR used by `cargo-orthohelp`.

mod fields;
mod sections;
mod types;

pub(crate) use types::AppName;

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::derive::build::CliFieldMetadata;
use crate::derive::parse::{
    DocExampleAttr, DocLinkAttr, DocNoteAttr, FieldAttrs, SerdeRenameAll, StructAttrs,
};

pub(crate) struct DocsArgs<'a> {
    pub ident: &'a Ident,
    pub fields: &'a [syn::Field],
    pub field_attrs: &'a [FieldAttrs],
    pub struct_attrs: &'a StructAttrs,
    pub serde_rename_all: Option<SerdeRenameAll>,
    pub cli_fields: &'a [CliFieldMetadata],
}

pub(crate) fn generate_docs_impl(args: &DocsArgs<'_>) -> syn::Result<TokenStream> {
    let app_name = sections::resolve_app_name(args.struct_attrs, args.ident);
    let app_name_value = AppName::new(app_name);
    let about_id = sections::resolve_about_id(&app_name_value, &args.struct_attrs.doc);
    let headings = sections::build_sections_metadata(&app_name_value, args.struct_attrs)?;
    let windows = sections::build_windows_metadata(args.struct_attrs);
    let fields = fields::build_fields_metadata(&fields::FieldDocArgs {
        app_name: &app_name_value,
        prefix: args.struct_attrs.prefix.as_deref(),
        fields: args.fields,
        field_attrs: args.field_attrs,
        serde_rename_all: args.serde_rename_all,
        cli_fields: args.cli_fields,
    })?;

    let app_name_lit = syn::LitStr::new(&app_name_value, proc_macro2::Span::call_site());
    let about_id_lit = syn::LitStr::new(&about_id, proc_macro2::Span::call_site());
    let bin_name_tokens = option_string_tokens(args.struct_attrs.doc.bin_name.as_deref());
    let synopsis_tokens = option_string_tokens(args.struct_attrs.doc.synopsis_id.as_deref());

    let ident = args.ident;

    Ok(quote! {
        impl ortho_config::docs::OrthoConfigDocs for #ident {
            fn get_doc_metadata() -> ortho_config::docs::DocMetadata {
                ortho_config::docs::DocMetadata {
                    ir_version: ortho_config::docs::ORTHO_DOCS_IR_VERSION.to_string(),
                    app_name: #app_name_lit.to_string(),
                    bin_name: #bin_name_tokens,
                    about_id: #about_id_lit.to_string(),
                    synopsis_id: #synopsis_tokens,
                    sections: #headings,
                    fields: vec![ #( #fields ),* ],
                    subcommands: Vec::new(),
                    windows: #windows,
                }
            }
        }
    })
}

pub(super) fn option_string_tokens(value: Option<&str>) -> TokenStream {
    value.map_or_else(
        || quote! { None },
        |text| {
            let lit = syn::LitStr::new(text, proc_macro2::Span::call_site());
            quote! { Some(String::from(#lit)) }
        },
    )
}

pub(super) fn option_char_tokens(value: Option<char>) -> TokenStream {
    value.map_or_else(
        || quote! { None },
        |ch| {
            let lit = syn::LitChar::new(ch, proc_macro2::Span::call_site());
            quote! { Some(#lit) }
        },
    )
}

pub(super) fn example_tokens(examples: &[DocExampleAttr]) -> Vec<TokenStream> {
    examples
        .iter()
        .map(|example| {
            let title_id = option_string_tokens(example.title_id.as_deref());
            let body_id = option_string_tokens(example.body_id.as_deref());
            let code = syn::LitStr::new(&example.code, proc_macro2::Span::call_site());
            quote! {
                ortho_config::docs::Example {
                    title_id: #title_id,
                    code: String::from(#code),
                    body_id: #body_id,
                }
            }
        })
        .collect()
}

pub(super) fn link_tokens(links: &[DocLinkAttr]) -> Vec<TokenStream> {
    links
        .iter()
        .map(|link| {
            let text_id = option_string_tokens(link.text_id.as_deref());
            let uri = syn::LitStr::new(&link.uri, proc_macro2::Span::call_site());
            quote! {
                ortho_config::docs::Link {
                    text_id: #text_id,
                    uri: String::from(#uri),
                }
            }
        })
        .collect()
}

pub(super) fn note_tokens(notes: &[DocNoteAttr]) -> Vec<TokenStream> {
    notes
        .iter()
        .map(|note| {
            let text_id = syn::LitStr::new(&note.text_id, proc_macro2::Span::call_site());
            quote! {
                ortho_config::docs::Note {
                    text_id: String::from(#text_id),
                }
            }
        })
        .collect()
}
