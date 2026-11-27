//! CLI metadata token generation for documentation fields.
//!
//! A field with an explicit clap declaration is described by clap's surface:
//! positionals carry an index and no flag spelling, and value names come from
//! the declaration or clap's casing rule. A field without one keeps the
//! generated `OrthoConfig` parser's surface, which the configuration loader
//! itself uses.

use proc_macro2::TokenStream;
use quote::quote;

use crate::derive::build::CliFieldMetadata;
use crate::derive::generate::docs::{option_char_tokens, option_string_tokens};

use super::clap_surface::ResolvedClapArg;
use super::shapes::FieldContext;
use super::tokens::build_possible_values;
use super::value_types::{ValueTypeModel, is_multi_value};

/// Builds one field's optional CLI metadata.
pub(super) fn build_cli_tokens(
    context: &FieldContext<'_>,
    clap: Option<&ResolvedClapArg>,
    generated: Option<&CliFieldMetadata>,
    krate: &TokenStream,
) -> syn::Result<TokenStream> {
    if let Some(decl) = clap {
        if decl.is_skipped {
            return Ok(quote! { None });
        }
        return Ok(clap_tokens(decl, context, krate));
    }
    if context.attrs.cli.is_skipped() {
        return Ok(quote! { None });
    }
    let meta = generated.ok_or_else(|| {
        syn::Error::new_spanned(
            context.name,
            "missing CLI metadata for field; this is a macro bug",
        )
    })?;
    Ok(generated_tokens(meta, context, krate))
}

/// Emits metadata for a field whose CLI surface clap declares.
fn clap_tokens(
    decl: &ResolvedClapArg,
    context: &FieldContext<'_>,
    krate: &TokenStream,
) -> TokenStream {
    let long = option_string_tokens(decl.long.as_deref());
    let short = option_char_tokens(decl.short);
    let value_name = option_string_tokens(
        context
            .attrs
            .doc
            .cli_value_name
            .as_deref()
            .or(Some(decl.value_name.as_str())),
    );
    let multiple = decl.multiple;
    let takes_value = decl.takes_value;
    let value_optional = decl.value_optional;
    let hide_in_help = decl.hide_in_help || context.attrs.doc.cli_hide_in_help;
    let possible_values = clap_possible_values(decl, context);
    let positional = decl.positional.as_ref().map_or_else(
        || quote! { None },
        |positional| {
            let index = positional.index;
            let variadic = positional.variadic;
            let var_arg = positional.var_arg;
            let last = positional.last;
            quote! {
                Some(
                    #krate::docs::PositionalMetadata::new(#index)
                        .with_variadic(#variadic)
                        .with_var_arg(#var_arg)
                        .with_last(#last),
                )
            }
        },
    );

    quote! {
        Some(#krate::docs::CliMetadata {
            long: #long,
            short: #short,
            value_name: #value_name,
            multiple: #multiple,
            takes_value: #takes_value,
            value_optional: #value_optional,
            possible_values: vec![ #( #possible_values ),* ],
            hide_in_help: #hide_in_help,
            positional: #positional,
        })
    }
}

/// Selects the possible values clap's surface exposes.
///
/// A value-taking boolean advertises its two literals, matching the generated
/// parser's optional-value form. A switch takes no value, so it has none.
fn clap_possible_values(decl: &ResolvedClapArg, context: &FieldContext<'_>) -> Vec<TokenStream> {
    if !decl.takes_value {
        return Vec::new();
    }
    if matches!(context.value_type, Some(ValueTypeModel::Bool)) {
        return vec![
            quote! { String::from("true") },
            quote! { String::from("false") },
        ];
    }
    build_possible_values(context.value_type)
}

/// Emits metadata for a field described by the generated configuration parser.
fn generated_tokens(
    meta: &CliFieldMetadata,
    context: &FieldContext<'_>,
    krate: &TokenStream,
) -> TokenStream {
    let long = option_string_tokens(Some(meta.long.as_str()));
    let short = option_char_tokens(Some(meta.short));
    let multiple = is_multi_value(&context.field.ty);
    let possible_values = build_possible_values(context.value_type);
    let hide_in_help = context.attrs.doc.cli_hide_in_help;

    // Boolean flags accept an optional `=<BOOL>` value. Report the value
    // placeholder and allowed literals so renderers can print the
    // `--flag[=<BOOL>]` form rather than an unexplained bare switch. Every
    // other option takes a required value, so only boolean flags mark their
    // value as optional.
    let (value_name, value_optional, resolved_possible_values) = if meta.is_bool {
        (
            option_string_tokens(Some("BOOL")),
            quote! { true },
            vec![
                quote! { String::from("true") },
                quote! { String::from("false") },
            ],
        )
    } else {
        (
            option_string_tokens(context.attrs.doc.cli_value_name.as_deref()),
            quote! { false },
            possible_values,
        )
    };

    quote! {
        Some(#krate::docs::CliMetadata {
            long: #long,
            short: #short,
            value_name: #value_name,
            multiple: #multiple,
            takes_value: true,
            value_optional: #value_optional,
            possible_values: vec![ #( #resolved_possible_values ),* ],
            hide_in_help: #hide_in_help,
            positional: None,
        })
    }
}
