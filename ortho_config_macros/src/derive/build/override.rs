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

/// Collects fields that use collection merge strategies.
///
/// Walks the parsed struct, capturing each named collection field configured
/// with a merge strategy. `Vec<_>` fields default to the append strategy, while
/// `BTreeMap<_, _>` fields default to keyed merges unless explicitly set to
/// replace.
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
        if let Some((ident, ty)) = process_vec_field(field, attrs)? {
            strategies.append.push((ident, ty));
            continue;
        }
        if let Some((ident, ty)) = process_map_field(field, attrs)? {
            strategies.map_replace.push((ident, ty));
            continue;
        }
        if attrs.merge_strategy.is_some() {
            return Err(syn::Error::new_spanned(
                field,
                "merge_strategy is only supported on Vec<_> or BTreeMap<_, _> fields",
            ));
        }
    }
    Ok(strategies)
}

fn process_vec_field(
    field: &syn::Field,
    attrs: &FieldAttrs,
) -> syn::Result<Option<(Ident, Type)>> {
    let Some(name) = field.ident.clone() else {
        return Err(syn::Error::new_spanned(
            field,
            "unnamed (tuple) fields do not support merge strategies",
        ));
    };
    let Some(vec_ty) = vec_inner(&field.ty) else {
        if matches!(attrs.merge_strategy, Some(MergeStrategy::Append)) {
            return Err(syn::Error::new_spanned(
                field,
                "append merge strategy requires a Vec<_> field",
            ));
        }
        return Ok(None);
    };

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

fn process_map_field(
    field: &syn::Field,
    attrs: &FieldAttrs,
) -> syn::Result<Option<(Ident, Type)>> {
    let Some(name) = field.ident.clone() else {
        return Err(syn::Error::new_spanned(
            field,
            "unnamed (tuple) fields do not support merge strategies",
        ));
    };
    if btree_map_inner(&field.ty).is_none() {
        return Ok(None);
    }

    let strategy = attrs.merge_strategy.unwrap_or(MergeStrategy::Keyed);
    match strategy {
        MergeStrategy::Append => Err(syn::Error::new_spanned(
            field,
            "append merge strategy is not supported for BTreeMap fields",
        )),
        MergeStrategy::Replace => Ok(Some((name, field.ty.clone()))),
        MergeStrategy::Keyed => Ok(None),
    }
}

/// Builds the override struct definition and initialisation expression.
///
/// Generates the hidden `__<Base>CollectionOverride` struct containing
/// `Option<Vec<T>>` fields for every append-enabled vector and
/// `Option<serde_json::Value>` fields for map replacements. Each field starts as
/// `None`.
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
    let map_fields = strategies.map_replace.iter().map(|(name, _)| {
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
    let definition = quote! {
        #[derive(serde::Serialize)]
        struct #ident {
            #( #struct_fields, )*
        }
    };
    let initialiser = quote! { #ident { #( #init, )* } };
    (definition, initialiser)
}

/// Aggregated token streams for collection merge logic.
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

fn build_map_state_decls(strategies: &CollectionStrategies) -> Vec<proc_macro2::TokenStream> {
    strategies
        .map_replace
        .iter()
        .map(|(name, ty)| {
            let state_ident = format_ident!("replace_{}", name);
            quote! { let mut #state_ident: Option<#ty> = None; }
        })
        .collect()
}

fn build_map_merge_blocks(strategies: &CollectionStrategies) -> Vec<proc_macro2::TokenStream> {
    strategies
        .map_replace
        .iter()
        .map(|(name, ty)| {
            let state_ident = format_ident!("replace_{}", name);
            quote! {
                if let Some(f) = &file_fig {
                    if let Ok(v) = f.extract_inner::<#ty>(stringify!(#name)) {
                        #state_ident = Some(v);
                    }
                }
                if let Ok(v) = env_figment.extract_inner::<#ty>(stringify!(#name)) {
                    #state_ident = Some(v);
                }
                if let Ok(v) = cli_figment.extract_inner::<#ty>(stringify!(#name)) {
                    #state_ident = Some(v);
                }
            }
        })
        .collect()
}

fn build_map_assignment_blocks(strategies: &CollectionStrategies) -> Vec<proc_macro2::TokenStream> {
    strategies
        .map_replace
        .iter()
        .map(|(name, _)| {
            let state_ident = format_ident!("replace_{}", name);
            quote! {
                if let Some(value) = #state_ident {
                    cfg.#name = value;
                }
            }
        })
        .collect()
}

fn build_pre_merge_tokens(strategies: &CollectionStrategies) -> proc_macro2::TokenStream {
    let append_logic = build_append_blocks(strategies);
    let map_state_decls = build_map_state_decls(strategies);
    let map_logic = build_map_merge_blocks(strategies);
    quote! {
        let cli_figment = ortho_config::figment::Figment::from(
            ortho_config::figment::providers::Serialized::defaults(&cli),
        );
        #( #map_state_decls )*
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
) -> CollectionLogicTokens {
    if strategies.append.is_empty() && strategies.map_replace.is_empty() {
        return CollectionLogicTokens {
            pre_merge: quote! {},
            post_extract: quote! {},
        };
    }

    let pre_merge = build_pre_merge_tokens(strategies);
    let post_extract = build_post_extract_tokens(strategies);
    CollectionLogicTokens {
        pre_merge,
        post_extract,
    }
}

#[cfg(test)]
mod tests;
