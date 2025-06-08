//! Procedural macros for `ortho_config`.
//!
//! The current implementation of the [`OrthoConfig`] derive provides a basic
//! `load` method that layers configuration from a `config.toml` file,
//! environment variables, and now command-line arguments via `clap`. CLI flag
//! names are automatically generated from `snake_case` field names using the
//! `kebab-case` convention.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Expr, Fields, GenericArgument, Lit, PathArguments, Type,
    parse_macro_input,
};

#[derive(Clone, Copy, PartialEq)]
enum MergeStrategy {
    Replace,
    Append,
}

impl MergeStrategy {
    fn parse(s: &str, span: proc_macro2::Span) -> Result<Self, syn::Error> {
        match s {
            "replace" => Ok(MergeStrategy::Replace),
            "append" => Ok(MergeStrategy::Append),
            _ => Err(syn::Error::new(span, "unknown merge_strategy")),
        }
    }
}

#[derive(Default)]
struct StructAttrs {
    prefix: Option<String>,
}

#[derive(Default)]
struct FieldAttrs {
    cli_long: Option<String>,
    cli_short: Option<char>,
    default: Option<Expr>,
    merge_strategy: Option<MergeStrategy>,
}

fn parse_struct_attrs(attrs: &[Attribute]) -> Result<StructAttrs, syn::Error> {
    let mut out = StructAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("ortho_config") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("prefix") {
                let val = meta.value()?.parse::<Lit>()?;
                if let Lit::Str(s) = val {
                    out.prefix = Some(s.value());
                } else {
                    return Err(syn::Error::new(val.span(), "prefix must be a string"));
                }
            }
            Ok(())
        })?;
    }
    Ok(out)
}

fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs, syn::Error> {
    let mut out = FieldAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("ortho_config") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("cli_long") {
                let val = meta.value()?.parse::<Lit>()?;
                if let Lit::Str(s) = val {
                    out.cli_long = Some(s.value());
                } else {
                    return Err(syn::Error::new(val.span(), "cli_long must be a string"));
                }
            } else if meta.path.is_ident("cli_short") {
                let val = meta.value()?.parse::<Lit>()?;
                if let Lit::Char(c) = val {
                    out.cli_short = Some(c.value());
                } else {
                    return Err(syn::Error::new(val.span(), "cli_short must be a char"));
                }
            } else if meta.path.is_ident("default") {
                let expr = meta.value()?.parse::<Expr>()?;
                out.default = Some(expr);
            } else if meta.path.is_ident("merge_strategy") {
                let val = meta.value()?.parse::<Lit>()?;
                if let Lit::Str(s) = val {
                    out.merge_strategy = Some(MergeStrategy::parse(&s.value(), s.span())?);
                } else {
                    return Err(syn::Error::new(
                        val.span(),
                        "merge_strategy must be a string",
                    ));
                }
            }
            Ok(())
        })?;
    }
    Ok(out)
}

fn option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

fn vec_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(p) = ty {
        if let Some(seg) = p.path.segments.last() {
            if seg.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

fn build_override_struct(
    base: &syn::Ident,
    fields: &[(syn::Ident, &Type)],
) -> (
    syn::Ident,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
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
    (ident, ts, init_ts)
}

fn build_append_logic(fields: &[(syn::Ident, &Type)]) -> proc_macro2::TokenStream {
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

/// Derive macro for [`ortho_config::OrthoConfig`].
#[proc_macro_derive(OrthoConfig, attributes(ortho_config))]
pub fn derive_ortho_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let struct_attrs = match parse_struct_attrs(&input.attrs) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(named) => named.named,
            _ => {
                return syn::Error::new_spanned(
                    data.struct_token,
                    "OrthoConfig requires named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(ident, "OrthoConfig can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let cli_ident = format_ident!("__{}Cli", ident);
    let cli_mod = format_ident!("__{}CliMod", ident);
    let cli_pub_ident = format_ident!("{}Cli", ident);

    let mut field_attrs = Vec::new();
    for f in &fields {
        match parse_field_attrs(&f.attrs) {
            Ok(a) => field_attrs.push(a),
            Err(e) => return e.to_compile_error().into(),
        }
    }

    let cli_fields = fields.iter().zip(field_attrs.iter()).map(|(f, attr)| {
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
    });

    let defaults_ident = format_ident!("__{}Defaults", ident);
    let default_struct_fields = fields.iter().map(|f| {
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
    });

    let default_struct_init = fields.iter().zip(field_attrs.iter()).map(|(f, attr)| {
        let name = f.ident.as_ref().expect("named field");
        if let Some(expr) = &attr.default {
            quote! { #name: Some(#expr) }
        } else {
            quote! { #name: None }
        }
    });

    let env_provider = if let Some(prefix) = &struct_attrs.prefix {
        quote! { Env::prefixed(#prefix) }
    } else {
        quote! { Env::raw() }
    };

    let append_fields: Vec<_> = fields
        .iter()
        .zip(field_attrs.iter())
        .filter_map(|(f, attr)| {
            let ty = &f.ty;
            let name = f.ident.as_ref().unwrap();
            let vec_ty = match vec_inner(ty) {
                Some(v) => v,
                None => return None,
            };
            let strategy = attr.merge_strategy.unwrap_or(MergeStrategy::Append);
            if strategy == MergeStrategy::Append {
                Some((name.clone(), vec_ty))
            } else {
                None
            }
        })
        .collect();

    let (_override_ident, override_struct_ts, override_init_ts) =
        build_override_struct(&ident, &append_fields);
    let append_logic = build_append_logic(&append_fields);

    let expanded = quote! {
        mod #cli_mod {
            use std::option::Option as Option;
            #[derive(clap::Parser, serde::Serialize)]
            #[command(rename_all = "kebab-case")]
            pub struct #cli_ident {
                #( #cli_fields, )*
            }
        }

        #[derive(serde::Serialize)]
        struct #defaults_ident {
            #( #default_struct_fields, )*
        }

        #override_struct_ts

        pub use #cli_mod::#cli_ident as #cli_pub_ident;

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

        impl ortho_config::OrthoConfig for #ident {
            fn load() -> Result<Self, ortho_config::OrthoError> {
                Self::load_from_iter(::std::env::args_os())
            }
        }
    };

    TokenStream::from(expanded)
}
