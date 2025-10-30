//! Override helpers that build collection structures for derived configuration.
//!
//! These utilities analyse collection merge strategies, generate the supporting
//! override struct, and emit load-time aggregation logic for vector append and
//! map replacement semantics.

use quote::{format_ident, quote};
use syn::{Ident, Type};

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
/// ```rust,no_run
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
/// ```rust,no_run
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
/// ```rust,no_run
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

fn build_append_blocks(strategies: &CollectionStrategies) -> Vec<proc_macro2::TokenStream> {
    strategies
        .append
        .iter()
        .map(|(name, ty)| {
            quote! {
                {
                    let mut vec_acc: Vec<#ty> = Vec::new();
                    if let Some(val) = &defaults.#name { vec_acc.extend(val.clone()); }
                    if let Some(f) = &file_fig {
                        if let Ok(v) = f.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                    }
                    if let Ok(v) = env_figment.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                    if let Ok(v) = cli_figment.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                    if !vec_acc.is_empty() {
                        overrides.#name = Some(vec_acc);
                    }
                }
            }
        })
        .collect()
}

fn build_map_state_tokens(strategies: &CollectionStrategies) -> Option<proc_macro2::TokenStream> {
    if strategies.map_replace.is_empty() {
        return None;
    }

    let fields = strategies.map_replace.iter().map(|(name, ty)| {
        quote! { #name: Option<#ty>, }
    });
    let init = strategies.map_replace.iter().map(|(name, _)| {
        quote! { #name: None }
    });
    Some(quote! {
        struct ReplaceState { #( #fields )* }
        let mut replace = ReplaceState { #( #init, )* };
    })
}

/// Context for extracting a collection field value from a figment source.
struct ExtractContext<'a> {
    name: &'a Ident,
    ty: &'a Type,
    state_field: proc_macro2::TokenStream,
}

/// Generate tokens for extracting a value and assigning it if non-empty.
fn build_inner_extract(
    figment_expr: &proc_macro2::TokenStream,
    context: &ExtractContext<'_>,
) -> proc_macro2::TokenStream {
    let name = context.name;
    let ty = context.ty;
    let state_field = &context.state_field;
    quote! {
        if let Ok(v) = #figment_expr.extract_inner::<#ty>(stringify!(#name)) {
            if !v.is_empty() {
                #state_field = Some(v);
            }
        }
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "the review request specifies an owned extractor expression"
)]
fn build_extraction_check(
    extractor_expr: proc_macro2::TokenStream,
    context: &ExtractContext<'_>,
) -> proc_macro2::TokenStream {
    build_inner_extract(&extractor_expr, context)
}

fn build_figment_extract(
    figment_binding: &proc_macro2::TokenStream,
    context: &ExtractContext<'_>,
    is_optional: bool,
) -> proc_macro2::TokenStream {
    if is_optional {
        let inner = build_extraction_check(quote! { f }, context);
        quote! {
            if let Some(f) = #figment_binding {
                #inner
            }
        }
    } else {
        build_extraction_check(figment_binding.clone(), context)
    }
}

fn build_map_merge_blocks(strategies: &CollectionStrategies) -> Vec<proc_macro2::TokenStream> {
    strategies
        .map_replace
        .iter()
        .map(|(name, ty)| {
            let state_field = quote! { replace.#name };
            let context = ExtractContext {
                name,
                ty,
                state_field,
            };
            let file_binding = quote! { &file_fig };
            let env_binding = quote! { env_figment };
            let cli_binding = quote! { cli_figment };
            let file_extract = build_figment_extract(&file_binding, &context, true);
            let env_extract = build_figment_extract(&env_binding, &context, false);
            let cli_extract = build_figment_extract(&cli_binding, &context, false);
            quote! {
                #file_extract
                #env_extract
                #cli_extract
            }
        })
        .collect()
}

fn build_map_assignment_blocks(strategies: &CollectionStrategies) -> Vec<proc_macro2::TokenStream> {
    strategies
        .map_replace
        .iter()
        .map(|(name, _ty)| {
            quote! {
                if let Some(value) = replace.#name.take() {
                    cfg.#name = value;
                }
            }
        })
        .collect()
}

fn build_pre_merge_tokens(
    strategies: &CollectionStrategies,
    cli_binding: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let append_logic = build_append_blocks(strategies);
    let map_state = build_map_state_tokens(strategies);
    let map_logic = build_map_merge_blocks(strategies);
    let map_state_ts = map_state.unwrap_or_default();
    quote! {
        #[allow(unused_mut)]
        let mut cli_figment = ortho_config::figment::Figment::new();
        if let Some(cli) = #cli_binding {
            cli_figment = ortho_config::figment::Figment::from(
                ortho_config::figment::providers::Serialized::defaults(cli),
            );
        }
        #map_state_ts
        #( #append_logic )*
        #( #map_logic )*
    }
}

fn build_post_extract_tokens(strategies: &CollectionStrategies) -> proc_macro2::TokenStream {
    let map_assignments = build_map_assignment_blocks(strategies);
    quote! { #( #map_assignments )* }
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
