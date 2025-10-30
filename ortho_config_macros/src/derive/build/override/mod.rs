//! Override helpers that build collection structures for derived configuration.
//!
//! These utilities analyse collection merge strategies, generate the supporting
//! override struct, and emit load-time aggregation logic for vector append and
//! map replacement semantics.

mod tokens;

use quote::{format_ident, quote};
use syn::{Ident, Type};

use self::tokens::{build_post_extract_tokens, build_pre_merge_tokens};

use crate::derive::parse::{FieldAttrs, MergeStrategy, btree_map_inner, vec_inner};

/// Collection strategy metadata gathered from struct fields.
#[derive(Default)]
pub(crate) struct CollectionStrategies {
    pub append: Vec<(Ident, Type)>,
    pub map_replace: Vec<(Ident, Type)>,
}

fn process_vec_field(
    field: &syn::Field,
    name: Ident,
    vec_ty: &Type,
    attrs: &FieldAttrs,
) -> syn::Result<Option<(Ident, Type)>> {
    let strategy = attrs.merge_strategy.unwrap_or(MergeStrategy::Append);
    match strategy {
        MergeStrategy::Append => Ok(Some((name, (*vec_ty).clone()))),
        MergeStrategy::Replace => Ok(None),
        MergeStrategy::Keyed => Err(syn::Error::new_spanned(
            field,
            "keyed merge strategy is not supported for Vec<_> fields",
        )),
    }
}

fn process_btree_map_field(
    field: &syn::Field,
    name: Ident,
    field_ty: &Type,
    attrs: &FieldAttrs,
) -> syn::Result<Option<(Ident, Type)>> {
    let strategy = attrs.merge_strategy.unwrap_or(MergeStrategy::Keyed);
    match strategy {
        MergeStrategy::Append => Err(syn::Error::new_spanned(
            field,
            "append merge strategy is not supported for BTreeMap fields",
        )),
        MergeStrategy::Replace => Ok(Some((name, field_ty.clone()))),
        MergeStrategy::Keyed => Ok(None),
    }
}

fn validate_non_collection_field(field: &syn::Field, attrs: &FieldAttrs) -> syn::Result<()> {
    if attrs.merge_strategy.is_some() {
        return Err(syn::Error::new_spanned(
            field,
            "merge_strategy is only supported on Vec<_> or BTreeMap<_, _> fields",
        ));
    }
    Ok(())
}

/// Collects fields that use collection merge strategies.
///
/// Walks the parsed struct, capturing each named collection field configured
/// with a merge strategy. `Vec<_>` fields default to the append strategy, while
/// `BTreeMap<_, _>` fields default to keyed merges unless explicitly set to
/// replace.
///
/// # Examples
///
/// The nightly-2025-09-16 toolchain that backs this repository currently ICEs
/// when compiling these doctests, so the example is marked `ignore` until the
/// regression is resolved.
///
/// ```rust,ignore
/// # use crate::derive::build::r#override::collect_collection_strategies;
/// # use crate::derive::parse::parse_input;
/// let input: syn::DeriveInput = syn::parse_quote! {
///     struct Demo {
///         #[ortho_config(merge_strategy = "append")]
///         values: Vec<String>,
///         #[ortho_config(merge_strategy = "replace")]
///         rules: std::collections::BTreeMap<String, usize>,
///     }
/// };
/// let (_, fields, _, field_attrs) =
///     parse_input(&input).expect("derive input should parse");
/// let strategies = collect_collection_strategies(&fields, &field_attrs)
///     .expect("expected strategies to parse");
/// assert_eq!(strategies.append.len(), 1);
/// assert_eq!(strategies.map_replace.len(), 1);
/// ```
///
/// # Errors
///
/// Returns an error when merge strategies are applied to unsupported types or
/// when tuple fields request strategy customisation.
pub(crate) fn collect_collection_strategies(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<CollectionStrategies> {
    let mut strategies = CollectionStrategies::default();
    for (field, attrs) in fields.iter().zip(field_attrs) {
        let Some(name) = field.ident.clone() else {
            return Err(syn::Error::new_spanned(
                field,
                "unnamed (tuple) fields do not support merge strategies",
            ));
        };
        if let Some(vec_ty) = vec_inner(&field.ty) {
            let Some((vec_name, ty)) = process_vec_field(field, name.clone(), vec_ty, attrs)?
            else {
                continue;
            };
            strategies.append.push((vec_name, ty));
            continue;
        }

        if btree_map_inner(&field.ty).is_some() {
            let Some((map_name, ty)) =
                process_btree_map_field(field, name.clone(), &field.ty, attrs)?
            else {
                continue;
            };
            strategies.map_replace.push((map_name, ty));
            continue;
        }

        validate_non_collection_field(field, attrs)?;
    }
    Ok(strategies)
}

/// Builds the override struct definition and initialisation expression.
///
/// Generates the hidden `__<Base>CollectionOverride` struct containing
/// `Option<Vec<T>>` fields for every append-enabled vector and
/// `Option<serde_json::Value>` fields for map replacements. Each field starts as
/// `None`.
///
/// # Examples
///
/// The nightly-2025-09-16 toolchain that backs this repository currently ICEs
/// when compiling these doctests, so the example is marked `ignore` until the
/// regression is resolved.
///
/// ```rust,ignore
/// # use crate::derive::build::r#override::{
/// #     build_override_struct, collect_collection_strategies
/// # };
/// # use crate::derive::parse::parse_input;
/// let input: syn::DeriveInput = syn::parse_quote! {
///     struct Demo {
///         #[ortho_config(merge_strategy = "append")]
///         values: Vec<String>,
///     }
/// };
/// let (_, fields, _, field_attrs) =
///     parse_input(&input).expect("derive input should parse");
/// let strategies = collect_collection_strategies(&fields, &field_attrs)
///     .expect("expected strategies to parse");
/// let (struct_def, init) = build_override_struct(
///     &syn::parse_quote!(Demo),
///     &strategies,
/// );
/// assert!(struct_def.to_string().contains("__DemoCollectionOverride"));
/// assert!(init.to_string().contains("None"));
/// ```
///
/// Returns a tuple containing the struct definition tokens and the
/// corresponding initialiser.
pub(crate) fn build_override_struct(
    base: &Ident,
    strategies: &CollectionStrategies,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let ident = format_ident!("__{}CollectionOverride", base);
    let vec_fields = strategies.append.iter().map(|(name, ty)| {
        quote! {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub #name: Option<Vec<#ty>>
        }
    });
    let map_fields = strategies.map_replace.iter().map(|(name, _ty)| {
        quote! {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub #name: Option<ortho_config::serde_json::Value>
        }
    });
    let struct_fields = vec_fields.chain(map_fields);
    let init_vec = strategies
        .append
        .iter()
        .map(|(name, _)| quote! { #name: None });
    let init_map = strategies
        .map_replace
        .iter()
        .map(|(name, _)| quote! { #name: None });
    let init = init_vec.chain(init_map);
    let ts = quote! {
        #[derive(serde::Serialize)]
        struct #ident {
            #( #struct_fields, )*
        }
    };
    let init_ts = quote! { #ident { #( #init, )* } };
    (ts, init_ts)
}

/// Builds the collection aggregation logic for override fields.
///
/// Produces load-time accumulation code that drains `Vec<T>` values from the
/// defaults, file, environment, and CLI layers in precedence order (append) and
/// captures the last non-empty map contribution for replace semantics.
///
/// # Examples
///
/// The nightly-2025-09-16 toolchain that backs this repository currently ICEs
/// when compiling these doctests, so the example is marked `ignore` until the
/// regression is resolved.
///
/// ```rust,ignore
/// # use crate::derive::build::r#override::{
/// #     build_collection_logic, collect_collection_strategies
/// # };
/// # use crate::derive::parse::parse_input;
/// use quote::quote;
/// let input: syn::DeriveInput = syn::parse_quote! {
///     struct Demo {
///         #[ortho_config(merge_strategy = "append")]
///         values: Vec<String>,
///     }
/// };
/// let (_, fields, _, field_attrs) =
///     parse_input(&input).expect("derive input should parse");
/// let strategies = collect_collection_strategies(&fields, &field_attrs)
///     .expect("expected strategies to parse");
/// let cli_binding = quote!(std::option::Option::<&()>::None);
/// let tokens = build_collection_logic(&strategies, &cli_binding);
/// assert!(!tokens.pre_merge.is_empty());
/// ```
///
/// Returns an empty token stream when no collections require special handling.
pub(crate) struct CollectionLogicTokens {
    pub pre_merge: proc_macro2::TokenStream,
    pub post_extract: proc_macro2::TokenStream,
}

pub(crate) fn build_collection_logic(
    strategies: &CollectionStrategies,
    cli_binding: &proc_macro2::TokenStream,
) -> CollectionLogicTokens {
    if strategies.append.is_empty() && strategies.map_replace.is_empty() {
        return CollectionLogicTokens {
            pre_merge: quote! {},
            post_extract: quote! {},
        };
    }

    let pre_merge = build_pre_merge_tokens(strategies, cli_binding);
    let post_extract = build_post_extract_tokens(strategies);
    CollectionLogicTokens {
        pre_merge,
        post_extract,
    }
}
