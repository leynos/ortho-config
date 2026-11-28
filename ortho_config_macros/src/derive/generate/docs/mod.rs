//! Documentation IR generation for `OrthoConfig`.
//!
//! This module emits `OrthoConfigDocs` implementations that return the
//! `DocMetadata` IR used by `cargo-orthohelp`.

mod behaviour;

mod entry;

mod discovery;

mod fields;

mod output;

mod projection;

mod renderer;

pub(crate) mod sections;

mod types;

pub(crate) use types::AppName;

use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;

use crate::derive::build::{CliFieldMetadata, compute_profile_env_var};
use crate::derive::generate::localization::LocalizationIds;
use crate::derive::parse::{
    DocExampleAttr, DocLinkAttr, DocNoteAttr, FieldAttrs, SerdeRenameAll, StructAttrs,
};

/// Borrowed derive inputs needed to emit the downstream documentation trait.
pub(crate) struct DocsArgs<'a> {
    /// Configuration type whose documentation metadata is being generated.
    pub ident: &'a Ident,
    /// Source fields, kept in declaration order for stable help output.
    pub fields: &'a [syn::Field],
    /// Parsed field attributes aligned with `fields` by index.
    pub field_attrs: &'a [FieldAttrs],
    /// Container settings that supply names, headings, and platform metadata.
    pub struct_attrs: &'a StructAttrs,
    /// Serde naming policy used to identify serialized configuration keys.
    pub serde_deserialize_rename_all: Option<SerdeRenameAll>,
    /// CLI details used to keep generated help metadata in sync with parsing.
    pub cli_fields: &'a [CliFieldMetadata],
    /// Localisation identifiers determine which field defaults follow the
    /// mounted command path. Fields absent from this model retain docs-only
    /// dotted defaults.
    pub localization_ids: &'a LocalizationIds,
    /// Resolved crate path for generated code references.
    pub krate: &'a TokenStream,
}

/// Emits an `OrthoConfigDocs` implementation whose IR combines parsed
/// attributes with the same CLI and Serde names used by runtime loading.
pub(crate) fn generate_docs_impl(args: &DocsArgs<'_>) -> syn::Result<TokenStream> {
    let tokens = entry::DocTokens::build(args)?;
    let helper = tokens.render_helper(args.ident);
    let trait_impl = tokens.render_trait_impl(args.ident);
    Ok(quote! {
        #helper
        #trait_impl
    })
}

/// Emits nested command metadata only when one supported subcommand field is
/// present; multiple selectors are rejected consistently with clap.
fn build_subcommands_metadata(args: &DocsArgs<'_>) -> syn::Result<TokenStream> {
    let subcommand_fields = args
        .fields
        .iter()
        .zip(args.field_attrs)
        .filter(|(_, attrs)| attrs.cli.is_subcommand())
        .map(|(field, _)| field)
        .collect::<Vec<_>>();

    match subcommand_fields.as_slice() {
        [] => Ok(quote! { Vec::new() }),
        [field] => {
            let inner_ty = unwrap_known_wrapper(&field.ty);
            let krate = args.krate;
            Ok(quote! {
                <#inner_ty as #krate::docs::OrthoConfigSubcommandDocs>
                    ::get_subcommand_doc_metadata_for_path(command_path)
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
///
/// This shallow projection keeps documentation generation independent of
/// arbitrary user-defined container semantics.
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

/// Turns an optional string into tokens for `Option<String>` without parsing
/// generated text back into Rust syntax.
pub(super) fn option_string_tokens(value: Option<&str>) -> TokenStream {
    value.map_or_else(
        || quote! { None },
        |text| {
            let lit = syn::LitStr::new(text, proc_macro2::Span::call_site());
            quote! { Some(String::from(#lit)) }
        },
    )
}

/// Turns an optional character into tokens for `Option<char>` while preserving
/// literal escaping through `syn`.
pub(super) fn option_char_tokens(value: Option<char>) -> TokenStream {
    value.map_or_else(
        || quote! { None },
        |ch| {
            let lit = syn::LitChar::new(ch, proc_macro2::Span::call_site());
            quote! { Some(#lit) }
        },
    )
}

/// Converts parsed examples into owned IR values in their source order.
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

/// Converts parsed links into IR tokens, retaining optional localized labels.
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

/// Converts note identifiers into owned IR values for the generated metadata.
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

/// Build the `profiles` IR metadata tokens (decision D15).
///
/// Opted-in structs emit `Some(DocProfilesMeta { flag, env_var })`; legacy
/// structs emit `None` so the IR stays additive and unchanged for them.
fn build_profiles_meta(args: &DocsArgs<'_>, krate: &TokenStream) -> TokenStream {
    if !args.struct_attrs.profiles {
        return quote! { None };
    }
    let env_var = syn::LitStr::new(
        &compute_profile_env_var(args.struct_attrs),
        proc_macro2::Span::call_site(),
    );
    quote! {
        Some(#krate::docs::DocProfilesMeta {
            flag: String::from("profile"),
            env_var: String::from(#env_var),
        })
    }
}
