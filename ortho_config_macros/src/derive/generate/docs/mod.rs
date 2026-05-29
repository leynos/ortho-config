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
    /// Resolved crate path for generated code references.
    pub krate: &'a TokenStream,
}

pub(crate) fn generate_docs_impl(args: &DocsArgs<'_>) -> syn::Result<TokenStream> {
    let krate = args.krate;
    let app_name = sections::resolve_app_name(args.struct_attrs, args.ident);
    let app_name_value = AppName::new(app_name);
    let about_id = sections::resolve_about_id(&app_name_value, &args.struct_attrs.doc);
    let headings = sections::build_sections_metadata(&app_name_value, args.struct_attrs, krate)?;
    let windows = sections::build_windows_metadata(args.struct_attrs, krate);
    let fields = fields::build_fields_metadata(&fields::FieldDocArgs {
        app_name: &app_name_value,
        prefix: args.struct_attrs.prefix.as_deref(),
        fields: args.fields,
        field_attrs: args.field_attrs,
        serde_rename_all: args.serde_rename_all,
        cli_fields: args.cli_fields,
        krate,
    })?;
    let subcommands = build_subcommands_metadata(args)?;

    let app_name_lit = syn::LitStr::new(&app_name_value, proc_macro2::Span::call_site());
    let about_id_lit = syn::LitStr::new(&about_id, proc_macro2::Span::call_site());
    let bin_name_tokens = option_string_tokens(args.struct_attrs.doc.bin_name.as_deref());
    let synopsis_tokens = option_string_tokens(args.struct_attrs.doc.synopsis_id.as_deref());

    let ident = args.ident;

    Ok(quote! {
        impl #krate::docs::OrthoConfigDocs for #ident {
            fn get_doc_metadata() -> #krate::docs::DocMetadata {
                #krate::docs::DocMetadata {
                    ir_version: #krate::docs::ORTHO_DOCS_IR_VERSION.to_string(),
                    app_name: #app_name_lit.to_string(),
                    bin_name: #bin_name_tokens,
                    about_id: #about_id_lit.to_string(),
                    synopsis_id: #synopsis_tokens,
                    sections: #headings,
                    fields: vec![ #( #fields ),* ],
                    subcommands: #subcommands,
                    windows: #windows,
                }
            }
        }
    })
}

fn build_subcommands_metadata(args: &DocsArgs<'_>) -> syn::Result<TokenStream> {
    let subcommand_fields = args
        .fields
        .iter()
        .zip(args.field_attrs)
        .filter(|(_, attrs)| attrs.is_subcommand)
        .map(|(field, _)| field)
        .collect::<Vec<_>>();

    match subcommand_fields.as_slice() {
        [] => Ok(quote! { Vec::new() }),
        [field] => {
            let inner_ty = unwrap_known_wrapper(&field.ty);
            let krate = args.krate;
            Ok(quote! {
                <#inner_ty as #krate::docs::OrthoConfigSubcommandDocs>
                    ::get_subcommand_doc_metadata()
            })
        }
        [first, ..] => Err(syn::Error::new_spanned(
            first,
            "multiple #[command(subcommand)] fields are not supported; clap also rejects multiple subcommand selectors on one struct",
        )),
    }
}

/// Unwrap `Option<T>` and `Vec<T>` to the inner type `T`, falling back to
/// the original type for unrecognized wrappers.
fn unwrap_known_wrapper(ty: &syn::Type) -> &syn::Type {
    let syn::Type::Path(type_path) = ty else {
        return ty;
    };
    let Some(last_segment) = type_path.path.segments.last() else {
        return ty;
    };
    let ident = last_segment.ident.to_string();
    if ident != "Option" && ident != "Vec" {
        return ty;
    }
    let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments else {
        return ty;
    };
    let Some(syn::GenericArgument::Type(inner)) = args.args.first() else {
        return ty;
    };
    inner
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

pub(super) fn example_tokens(examples: &[DocExampleAttr], krate: &TokenStream) -> Vec<TokenStream> {
    examples
        .iter()
        .map(|example| {
            let title_id = option_string_tokens(example.title_id.as_deref());
            let body_id = option_string_tokens(example.body_id.as_deref());
            let code = syn::LitStr::new(&example.code, proc_macro2::Span::call_site());
            quote! {
                #krate::docs::Example {
                    title_id: #title_id,
                    code: String::from(#code),
                    body_id: #body_id,
                }
            }
        })
        .collect()
}

pub(super) fn link_tokens(links: &[DocLinkAttr], krate: &TokenStream) -> Vec<TokenStream> {
    links
        .iter()
        .map(|link| {
            let text_id = option_string_tokens(link.text_id.as_deref());
            let uri = syn::LitStr::new(&link.uri, proc_macro2::Span::call_site());
            quote! {
                #krate::docs::Link {
                    text_id: #text_id,
                    uri: String::from(#uri),
                }
            }
        })
        .collect()
}

pub(super) fn note_tokens(notes: &[DocNoteAttr], krate: &TokenStream) -> Vec<TokenStream> {
    notes
        .iter()
        .map(|note| {
            let text_id = syn::LitStr::new(&note.text_id, proc_macro2::Span::call_site());
            quote! {
                #krate::docs::Note {
                    text_id: String::from(#text_id),
                }
            }
        })
        .collect()
}
