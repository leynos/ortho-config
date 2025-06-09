use quote::{format_ident, quote};
use syn::{Ident, Type};

use super::parse::{FieldAttrs, StructAttrs, option_inner, vec_inner};

pub(crate) fn build_cli_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .zip(field_attrs.iter())
        .map(|(f, attr)| {
            let name = f.ident.as_ref().expect("named field");
            let ty = &f.ty;
            let inner = option_inner(ty);
            let cli_ty = if let Some(inner) = inner {
                quote! { Option<#inner> }
            } else {
                quote! { Option<#ty> }
            };

            let mut arg_tokens = quote! { long };
            if let Some(ref long) = attr.cli_long {
                arg_tokens = quote! { long = #long };
            }
            if let Some(ch) = attr.cli_short {
                let short_token = quote! { short = #ch };
                arg_tokens = quote! { #arg_tokens, #short_token };
            }

            quote! {
                #[arg(#arg_tokens, required = false)]
                #[serde(skip_serializing_if = "Option::is_none")]
                pub #name: #cli_ty
            }
        })
        .collect()
}

pub(crate) fn build_default_struct_fields(fields: &[syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|f| {
            let name = f.ident.as_ref().expect("named field");
            let ty = &f.ty;
            let inner = option_inner(ty);
            let default_ty = if let Some(inner) = inner {
                quote! { Option<#inner> }
            } else {
                quote! { Option<#ty> }
            };
            quote! {
                #[serde(skip_serializing_if = "Option::is_none")]
                pub #name: #default_ty
            }
        })
        .collect()
}

pub(crate) fn build_default_struct_init(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .zip(field_attrs.iter())
        .map(|(f, attr)| {
            let name = f.ident.as_ref().expect("named field");
            if let Some(expr) = &attr.default {
                quote! { #name: Some(#expr) }
            } else {
                quote! { #name: None }
            }
        })
        .collect()
}

pub(crate) fn build_env_provider(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    if let Some(prefix) = &struct_attrs.prefix {
        quote! { Env::prefixed(#prefix) }
    } else {
        quote! { Env::raw() }
    }
}

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
            let strategy = attr
                .merge_strategy
                .unwrap_or(super::parse::MergeStrategy::Append);
            if strategy == super::parse::MergeStrategy::Append {
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
    let logic = fields.iter().map(|(name, ty)| {
        quote! {
            {
                let mut vec_acc: Vec<#ty> = Vec::new();
                if let Some(val) = &defaults.#name { vec_acc.extend(val.clone()); }
                if let Some(f) = &file_fig {
                    if let Ok(v) = f.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                }
                if let Ok(v) = env_fig.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                if let Ok(v) = cli_fig.extract_inner::<Vec<#ty>>(stringify!(#name)) { vec_acc.extend(v); }
                if !vec_acc.is_empty() {
                    overrides.#name = Some(vec_acc);
                }
            }
        }
    });
    quote! { #( #logic )* }
}

pub(crate) struct LoadImplArgs<'a> {
    pub ident: &'a Ident,
    pub cli_mod: &'a Ident,
    pub cli_ident: &'a Ident,
    pub defaults_ident: &'a Ident,
    pub env_provider: &'a proc_macro2::TokenStream,
    pub default_struct_init: &'a [proc_macro2::TokenStream],
    pub override_init_ts: &'a proc_macro2::TokenStream,
    pub append_logic: &'a proc_macro2::TokenStream,
}

pub(crate) fn build_load_impl(args: &LoadImplArgs<'_>) -> proc_macro2::TokenStream {
    let LoadImplArgs {
        ident,
        cli_mod,
        cli_ident,
        defaults_ident,
        env_provider,
        default_struct_init,
        override_init_ts,
        append_logic,
    } = args;

    quote! {
        impl #ident {
            #[allow(dead_code)]
            pub fn load_from_iter<I>(args: I) -> Result<Self, ortho_config::OrthoError>
            where
                I: IntoIterator,
                I::Item: AsRef<std::ffi::OsStr>,
            {
                use clap::Parser as _;
                use figment::{Figment, providers::{Toml, Env, Serialized, Format}, Profile};
                #[cfg(feature = "json")] use figment::providers::Json;
                #[cfg(feature = "yaml")] use figment::providers::Yaml;
                use uncased::Uncased;
                #[cfg(feature = "json")] use serde_json;
                #[cfg(feature = "yaml")] use serde_yaml;
                #[cfg(feature = "toml")] use toml;

                let cli = #cli_mod::#cli_ident::try_parse_from(
                    args.into_iter().map(|a| a.as_ref().to_os_string())
                )
                .map_err(ortho_config::OrthoError::CliParsing)?;

                let cfg_path = std::env::var("CONFIG_PATH")
                    .unwrap_or_else(|_| "config.toml".to_string());

                let mut fig = Figment::new();
                let defaults = #defaults_ident {
                    #( #default_struct_init, )*
                };

                let mut overrides = #override_init_ts;

                fig = fig.merge(Serialized::defaults(&defaults));

                let file_fig = ortho_config::load_config_file(std::path::Path::new(&cfg_path))?;
                if let Some(ref f) = file_fig {
                    fig = fig.merge(f.clone());
                }

                let env_provider = {
                    #env_provider
                        .map(|k| Uncased::new(k.as_str().to_ascii_uppercase()))
                        .split("__")
                };

                let env_fig = Figment::from(env_provider.clone());
                let cli_fig = Figment::from(Serialized::from(&cli, Profile::Default));

                fig = fig.merge(env_provider).merge(Serialized::from(&cli, Profile::Default));

                #append_logic

                fig = fig.merge(Serialized::defaults(overrides));

                fig.extract().map_err(ortho_config::OrthoError::Gathering)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    fn demo_input() -> (Vec<syn::Field>, Vec<FieldAttrs>, StructAttrs) {
        let input: syn::DeriveInput = parse_quote! {
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
    fn env_provider_tokens() {
        let (_, _, struct_attrs) = demo_input();
        let ts = build_env_provider(&struct_attrs);
        assert_eq!(ts.to_string(), "Env :: prefixed (\"CFG_\")");
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
