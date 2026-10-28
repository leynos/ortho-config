//! Token emission for the optional `renderer` IR block.
//!
//! Kept in its own module so `sections.rs` and `generate/docs/mod.rs` stay
//! under the repository's 400-line ceiling.
//!
//! Every parser-accepted token has an explicit arm here, and an unrecognized
//! one emits nothing rather than inventing a value: the parser has already
//! rejected invalid spellings, so the only remaining state is "undeclared".

use proc_macro2::TokenStream;
use quote::quote;

use crate::derive::parse::{
    DocStructAttrs, RenderWidthAttr, RendererFieldAttr, RendererRecordAttrs,
};

/// Builds the token stream for the optional `renderer` IR block.
///
/// Returns `None` when no `renderer(...)` declaration exists, so consumers keep
/// seeing the undeclared state rather than an inferred value.
pub(super) fn build_renderer_metadata(doc: &DocStructAttrs, krate: &TokenStream) -> TokenStream {
    let Some(renderer) = doc.renderer.as_ref() else {
        return quote! { None };
    };
    let human = renderer
        .human
        .as_ref()
        .map_or_else(|| quote! { None }, |record| record_tokens(record, krate));
    let machine = renderer
        .machine
        .as_ref()
        .map_or_else(|| quote! { None }, |record| record_tokens(record, krate));

    quote! {
        Some(#krate::docs::RendererMetadata {
            human: #human,
            machine: #machine,
        })
    }
}

/// Emits `Some(RendererRecord { .. })` for one declared renderer record.
fn record_tokens(record: &RendererRecordAttrs, krate: &TokenStream) -> TokenStream {
    let supported = bool_option_tokens(record.supported);
    let tty = enum_option_tokens(record.tty.as_deref(), "TtyPolicy", krate);
    let closed_stdin =
        enum_option_tokens(record.closed_stdin.as_deref(), "ClosedStdinPolicy", krate);
    let colour = enum_option_tokens(record.colour.as_deref(), "TogglePolicy", krate);
    let emoji = enum_option_tokens(record.emoji.as_deref(), "TogglePolicy", krate);
    let progress = enum_option_tokens(record.progress.as_deref(), "TogglePolicy", krate);
    let pager = enum_option_tokens(record.pager.as_deref(), "TogglePolicy", krate);
    let width = record
        .width
        .as_ref()
        .map_or_else(|| quote! { None }, |width| width_tokens(width, krate));
    let accessibility = enum_option_tokens(
        record.accessibility.as_deref(),
        "AccessibilityPolicy",
        krate,
    );
    let plain = enum_option_tokens(record.plain.as_deref(), "PlainPolicy", krate);
    let stdout = enum_option_tokens(record.stdout.as_deref(), "StreamContent", krate);
    let stderr = enum_option_tokens(record.stderr.as_deref(), "StreamContent", krate);
    let progress_inheritance = enum_option_tokens(
        record.progress_inheritance.as_deref(),
        "InheritancePolicy",
        krate,
    );
    let subprocess_inheritance = enum_option_tokens(
        record.subprocess_inheritance.as_deref(),
        "InheritancePolicy",
        krate,
    );
    let fields = record
        .fields
        .iter()
        .map(|field| field_tokens(field, krate))
        .collect::<Vec<_>>();

    quote! {
        Some(#krate::docs::RendererRecord {
            supported: #supported,
            tty: #tty,
            closed_stdin: #closed_stdin,
            colour: #colour,
            emoji: #emoji,
            progress: #progress,
            pager: #pager,
            width: #width,
            accessibility: #accessibility,
            plain: #plain,
            stdout: #stdout,
            stderr: #stderr,
            progress_inheritance: #progress_inheritance,
            subprocess_inheritance: #subprocess_inheritance,
            fields: vec![ #( #fields ),* ],
        })
    }
}

/// Emits an `Option<bool>` initialiser for a declared support value.
fn bool_option_tokens(value: Option<bool>) -> TokenStream {
    value.map_or_else(|| quote! { None }, |declared| quote! { Some(#declared) })
}

/// Emits an `Option<enum>` initialiser from a validated token and type name.
///
/// The token is converted to a variant name by splitting on `_` and
/// capitalizing each segment, which is exactly the `snake_case` spelling the
/// parser accepted. An unrecognized token emits `None` so the generator stays
/// total; the parser is the gate that prevents it.
fn enum_option_tokens(value: Option<&str>, type_name: &str, krate: &TokenStream) -> TokenStream {
    let Some(token) = value else {
        return quote! { None };
    };
    let type_ident = syn::Ident::new(type_name, proc_macro2::Span::call_site());
    let Some(variant) = variant_ident(token) else {
        return quote! { None };
    };
    quote! { Some(#krate::docs::#type_ident::#variant) }
}

/// Converts a `snake_case` wire token into a `CamelCase` variant identifier.
///
/// Returns `None` for an empty token, so a malformed value cannot produce an
/// empty identifier.
fn variant_ident(token: &str) -> Option<syn::Ident> {
    let mut name = String::new();
    for segment in token.split('_') {
        if segment.is_empty() {
            return None;
        }
        let mut chars = segment.chars();
        let first = chars.next()?;
        name.push(first.to_ascii_uppercase());
        name.push_str(chars.as_str());
    }
    Some(syn::Ident::new(&name, proc_macro2::Span::call_site()))
}

/// Emits an `Option<RenderWidth>` initialiser for a declared width.
fn width_tokens(width: &RenderWidthAttr, krate: &TokenStream) -> TokenStream {
    match width {
        RenderWidthAttr::Unbounded => quote! { Some(#krate::docs::RenderWidth::Unbounded) },
        RenderWidthAttr::Terminal => quote! { Some(#krate::docs::RenderWidth::Terminal) },
        RenderWidthAttr::Fixed(columns) => {
            let literal = proc_macro2::Literal::u16_unsuffixed(*columns);
            // The column count reaches a `NonZeroU16` through a `const` block:
            // the parser rejects zero, and if that guarantee ever broke the
            // const evaluation would fail at compile time rather than emit a
            // fabricated width.
            quote! {
                {
                    const COLUMNS: ::core::num::NonZeroU16 =
                        match ::core::num::NonZeroU16::new(#literal) {
                            ::core::option::Option::Some(value) => value,
                            ::core::option::Option::None => {
                                panic!("declared width must be non-zero")
                            }
                        };
                    Some(#krate::docs::RenderWidth::Fixed { columns: COLUMNS })
                }
            }
        }
    }
}

/// Emits one `RendererField` entry for a declared output field.
fn field_tokens(field: &RendererFieldAttr, krate: &TokenStream) -> TokenStream {
    let id = syn::LitStr::new(&field.id, proc_macro2::Span::call_site());
    // The IR's localization enum has no `Unknown` fallback, so the two accepted
    // tokens are matched explicitly rather than defaulted.
    let localization = if field.localization == "localized" {
        quote! { #krate::docs::RendererFieldLocalization::Localized }
    } else {
        quote! { #krate::docs::RendererFieldLocalization::NonLocalized }
    };
    quote! {
        #krate::docs::RendererField {
            id: String::from(#id),
            localization: #localization,
        }
    }
}
