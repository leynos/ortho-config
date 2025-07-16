//! Code generation helpers for the `OrthoConfig` derive macro.
//!
//! This module contains utilities shared across the code generation
//! routines. The `load_impl` submodule houses the helpers that build the
//! `load_from_iter` implementation used by the derive macro.

use quote::{format_ident, quote};
use syn::{Ident, Type};

use super::parse::{FieldAttrs, StructAttrs, option_inner, vec_inner};

fn option_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    if let Some(inner) = option_inner(ty) {
        quote! { Option<#inner> }
    } else {
        quote! { Option<#ty> }
    }
}

/// Generates the fields for the hidden `clap::Parser` struct.
///
/// Each user field becomes `Option<T>` to record whether the CLI provided
/// a value. This lets the configuration merge logic keep track of which
/// layer supplied each setting. A dedicated `config_path` field is
/// inserted to allow overriding the path to the configuration file.
///
/// This function is used internally by the derive macro to transform
/// user-defined struct fields into CLI-compatible equivalents.
pub(crate) fn build_cli_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> Vec<proc_macro2::TokenStream> {
    let mut out = Vec::new();
    out.push(quote! {
        #[arg(long = "config-path")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub config_path: Option<std::path::PathBuf>
    });

    out.extend(fields.iter().zip(field_attrs.iter()).map(|(f, attr)| {
        let name = f.ident.as_ref().expect("named field");
        let ty = option_type_tokens(&f.ty);

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
            pub #name: #ty
        }
    }));

    out
}

pub(crate) fn build_default_struct_fields(fields: &[syn::Field]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|f| {
            let name = f.ident.as_ref().expect("named field");
            let ty = option_type_tokens(&f.ty);
            quote! {
                #[serde(skip_serializing_if = "Option::is_none")]
                pub #name: #ty
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

pub(crate) fn build_config_env_var(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    if let Some(prefix) = &struct_attrs.prefix {
        let var = format!("{prefix}CONFIG_PATH");
        quote! { #var }
    } else {
        quote! { "CONFIG_PATH" }
    }
}

pub(crate) fn build_dotfile_name(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    let base = if let Some(prefix) = &struct_attrs.prefix {
        let base = prefix.trim_end_matches('_').to_ascii_lowercase();
        format!(".{base}.toml")
    } else {
        ".config.toml".to_string()
    };
    quote! { #base }
}

pub(crate) fn build_xdg_snippet(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    let prefix_lit = struct_attrs.prefix.as_deref().unwrap_or("");
    quote! {
        #[cfg(any(unix, target_os = "redox"))]
        if file_fig.is_none() {
            let xdg_base = ortho_config::normalize_prefix(#prefix_lit);
            let xdg_dirs = if xdg_base.is_empty() {
                xdg::BaseDirectories::new()
            } else {
                xdg::BaseDirectories::with_prefix(&xdg_base)
            };
            if let Some(p) = xdg_dirs.find_config_file("config.toml") {
                file_fig = ortho_config::load_config_file(&p)?;
            }
            #[cfg(feature = "json5")]
            if file_fig.is_none() {
                for ext in &["json", "json5"] {
                    let filename = format!("config.{}", ext);
                    if let Some(p) = xdg_dirs.find_config_file(&filename) {
                        file_fig = ortho_config::load_config_file(&p)?;
                        break;
                    }
                }
            }
            #[cfg(feature = "yaml")]
            if file_fig.is_none() {
                for ext in &["yaml", "yml"] {
                    let filename = format!("config.{}", ext);
                    if let Some(p) = xdg_dirs.find_config_file(&filename) {
                        file_fig = ortho_config::load_config_file(&p)?;
                        break;
                    }
                }
            }
        }
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
        let env_figment = Figment::from(env_provider.clone());
        let cli_figment = Figment::from(Serialized::from(&cli, Profile::Default));
        #( #logic )*
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
