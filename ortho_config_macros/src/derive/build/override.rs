//! Override helpers that build append structures for derived configuration.
//!
//! These utilities gather `Vec<_>` fields using append merge strategies and
//! generate the supporting override struct plus load-time aggregation logic.

use quote::{format_ident, quote};
use syn::{Ident, Type};

use crate::derive::parse::{FieldAttrs, MergeStrategy, vec_inner};

pub(crate) fn collect_append_fields<'a>(
    fields: &'a [syn::Field],
    field_attrs: &'a [FieldAttrs],
) -> Vec<(Ident, &'a Type)> {
    fields
        .iter()
        .zip(field_attrs.iter())
        .filter_map(|(f, attr)| {
            let ty = &f.ty;
            let name = f.ident.as_ref().unwrap();
            let vec_ty = vec_inner(ty)?;
            let strategy = attr.merge_strategy.unwrap_or(MergeStrategy::Append);
            if strategy == MergeStrategy::Append {
                Some((name.clone(), vec_ty))
            } else {
                None
            }
        })
        .collect()
}

pub(crate) fn build_override_struct(
    base: &Ident,
    fields: &[(Ident, &Type)],
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

pub(crate) fn build_append_logic(fields: &[(Ident, &Type)]) -> proc_macro2::TokenStream {
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
        let cli_figment = Figment::from(Serialized::defaults(&cli));
        #( #logic )*
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derive::parse::StructAttrs;

    fn demo_input() -> (Vec<syn::Field>, Vec<FieldAttrs>, StructAttrs) {
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
            crate::derive::parse::parse_input(&input).expect("parse_input");
        (fields, field_attrs, struct_attrs)
    }

    #[test]
    fn collect_append_fields_selects_vec_fields() {
        let (fields, field_attrs, _) = demo_input();
        let out = collect_append_fields(&fields, &field_attrs);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0.to_string(), "field2");
    }

    #[test]
    fn build_override_struct_creates_struct() {
        let (fields, field_attrs, _) = demo_input();
        let append = collect_append_fields(&fields, &field_attrs);
        let (ts, init_ts) = build_override_struct(&syn::parse_quote!(Demo), &append);
        assert!(ts.to_string().contains("struct __DemoVecOverride"));
        assert!(init_ts.to_string().contains("__DemoVecOverride"));
    }
}
