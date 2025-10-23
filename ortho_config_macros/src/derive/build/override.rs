//! Override helpers that build append structures for derived configuration.
//!
//! These utilities gather `Vec<_>` fields using append merge strategies and
//! generate the supporting override struct plus load-time aggregation logic.

use quote::{format_ident, quote};
use syn::{Ident, Type};

use crate::derive::parse::{FieldAttrs, MergeStrategy, vec_inner};

/// Collects fields that use the append merge strategy.
///
/// Walks the parsed struct, capturing each named `Vec<_>` field configured with
/// the append merge strategy—either explicitly or via the implicit default for
/// vectors—and returns the owned identifier and element type.
///
/// # Examples
///
/// ```rust,ignore
/// # use crate::derive::build::r#override::collect_append_fields;
/// # use crate::derive::parse::parse_input;
/// let input: syn::DeriveInput = syn::parse_quote! {
///     struct Demo {
///         #[ortho_config(merge_strategy = "append")]
///         values: Vec<String>,
///     }
/// };
/// let (_, fields, _, field_attrs) = parse_input(&input).unwrap();
/// let append = collect_append_fields(&fields, &field_attrs).unwrap();
/// assert_eq!(append.len(), 1);
/// assert_eq!(
///     append[0].0,
///     syn::parse_str::<syn::Ident>("values").unwrap()
/// );
/// ```
///
/// # Errors
///
/// Returns an error when an append strategy is requested for a field without a
/// `Vec<_>` type or when an unnamed (tuple) field requests append merging.
pub(crate) fn collect_append_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<Vec<(Ident, Type)>> {
    let mut append_fields = Vec::new();
    for (field, attrs) in fields.iter().zip(field_attrs) {
        let Some(name) = field.ident.clone() else {
            return Err(syn::Error::new_spanned(
                field,
                "unnamed (tuple) fields are not supported for append merge strategy",
            ));
        };
        let strategy = attrs.merge_strategy.unwrap_or(MergeStrategy::Append);
        let Some(vec_ty) = vec_inner(&field.ty) else {
            if matches!(attrs.merge_strategy, Some(MergeStrategy::Append)) {
                return Err(syn::Error::new_spanned(
                    field,
                    "append merge strategy requires a Vec<_> field",
                ));
            }
            continue;
        };
        if strategy == MergeStrategy::Append {
            append_fields.push((name, (*vec_ty).clone()));
        }
    }
    Ok(append_fields)
}

/// Builds the override struct definition and initialisation expression.
///
/// Generates the hidden `__<Base>VecOverride` struct containing
/// `Option<Vec<T>>` fields for every append-enabled vector and an initialiser
/// expression where each field starts as `None`.
///
/// # Examples
///
/// ```rust,ignore
/// # use crate::derive::build::r#override::{
/// #     build_override_struct, collect_append_fields
/// # };
/// # use crate::derive::parse::parse_input;
/// let input: syn::DeriveInput = syn::parse_quote! {
///     struct Demo {
///         #[ortho_config(merge_strategy = "append")]
///         values: Vec<String>,
///     }
/// };
/// let (_, fields, _, field_attrs) = parse_input(&input).unwrap();
/// let append = collect_append_fields(&fields, &field_attrs).unwrap();
/// let (struct_def, init) = build_override_struct(
///     &syn::parse_quote!(Demo),
///     &append,
/// );
/// assert!(struct_def.to_string().contains("__DemoVecOverride"));
/// assert!(init.to_string().contains("None"));
/// ```
///
/// Returns a tuple containing the struct definition tokens and the
/// corresponding initialiser.
pub(crate) fn build_override_struct(
    base: &Ident,
    fields: &[(Ident, Type)],
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let ident = format_ident!("__{}VecOverride", base);
    let struct_fields = fields.iter().map(|(name, ty)| {
        quote! {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub #name: Option<Vec<#ty>>
        }
    });
    let init = fields.iter().map(|(name, _)| quote! { #name: None });
    let ts = quote! {
        #[derive(serde::Serialize)]
        struct #ident {
            #( #struct_fields, )*
        }
    };
    let init_ts = quote! { #ident { #( #init, )* } };
    (ts, init_ts)
}

/// Builds the append accumulation logic for vector fields.
///
/// Produces load-time accumulation code that drains `Vec<T>` values from the
/// defaults, file, environment, and CLI layers in precedence order, combining
/// them into the override struct only when a layer contributes data.
///
/// # Examples
///
/// ```rust,ignore
/// # use crate::derive::build::r#override::{
/// #     build_append_logic, collect_append_fields
/// # };
/// # use crate::derive::parse::parse_input;
/// let input: syn::DeriveInput = syn::parse_quote! {
///     struct Demo {
///         #[ortho_config(merge_strategy = "append")]
///         values: Vec<String>,
///     }
/// };
/// let (_, fields, _, field_attrs) = parse_input(&input).unwrap();
/// let append = collect_append_fields(&fields, &field_attrs).unwrap();
/// let tokens = build_append_logic(&append);
/// assert!(!tokens.is_empty());
/// ```
///
/// Returns an empty token stream when no fields require append accumulation.
pub(crate) fn build_append_logic(fields: &[(Ident, Type)]) -> proc_macro2::TokenStream {
    if fields.is_empty() {
        return quote! {};
    }

    let logic = fields.iter().map(|(name, ty)| {
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
    });
    quote! {
        let cli_figment = ortho_config::figment::Figment::from(
            ortho_config::figment::providers::Serialized::defaults(&cli),
        );
        #( #logic )*
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derive::parse::StructAttrs;
    use anyhow::{Result, anyhow, ensure};

    fn demo_input() -> Result<(Vec<syn::Field>, Vec<FieldAttrs>, StructAttrs)> {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[ortho_config(prefix = "CFG_")]
            struct Demo {
                #[ortho_config(cli_long = "opt", cli_short = 'o', default = 5)]
                field1: Option<u32>,
                #[ortho_config(merge_strategy = "append")]
                field2: Vec<String>,
            }
        };
        let (_, fields, struct_attrs, field_attrs) =
            crate::derive::parse::parse_input(&input).map_err(|err| anyhow!(err))?;
        Ok((fields, field_attrs, struct_attrs))
    }

    #[test]
    fn collect_append_fields_selects_vec_fields() -> Result<()> {
        let (fields, field_attrs, _) = demo_input()?;
        let out = collect_append_fields(&fields, &field_attrs).map_err(|err| anyhow!(err))?;
        ensure!(out.len() == 1, "expected single append field");
        let (ident, _) = out
            .first()
            .ok_or_else(|| anyhow!("expected vector entry"))?;
        ensure!(ident == "field2", "expected field2 append target");
        Ok(())
    }

    #[test]
    fn build_override_struct_creates_struct() -> Result<()> {
        let (fields, field_attrs, _) = demo_input()?;
        let append = collect_append_fields(&fields, &field_attrs).map_err(|err| anyhow!(err))?;
        let (ts, init_ts) = build_override_struct(&syn::parse_quote!(Demo), &append);
        ensure!(
            ts.to_string().contains("struct __DemoVecOverride"),
            "override struct missing expected identifier"
        );
        ensure!(
            init_ts.to_string().contains("__DemoVecOverride"),
            "override init missing expected struct"
        );
        Ok(())
    }

    #[test]
    fn collect_append_fields_errors_on_non_vec_append() -> Result<()> {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct DemoAppendError {
                #[ortho_config(merge_strategy = "append")]
                field1: u32,
            }
        };
        let (_, fields, _, field_attrs) =
            crate::derive::parse::parse_input(&input).map_err(|err| anyhow!(err))?;
        let Err(err) = collect_append_fields(&fields, &field_attrs) else {
            return Err(anyhow!("expected append strategy validation to fail"));
        };
        ensure!(
            err.to_string()
                .contains("append merge strategy requires a Vec<_> field"),
            "unexpected error: {err}"
        );
        Ok(())
    }
}
