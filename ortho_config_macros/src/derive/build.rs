//! Code generation helpers for the `OrthoConfig` derive macro.
//!
//! This module contains utilities shared across the code generation
//! routines. The `load_impl` submodule houses the helpers that build the
//! `load_from_iter` implementation used by the derive macro.

use quote::{format_ident, quote};
use syn::{Ident, Type};

use super::parse::{FieldAttrs, StructAttrs, option_inner, vec_inner};
use std::collections::HashSet;

const RESERVED_SHORTS: &[char] = &['h', 'V'];
const RESERVED_LONGS: &[&str] = &["help", "version"];

fn option_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    if let Some(inner) = option_inner(ty) {
        quote! { Option<#inner> }
    } else {
        quote! { Option<#ty> }
    }
}

/// Resolves a short CLI flag ensuring uniqueness and validity.
///
/// # Examples
///
/// ```ignore
/// use std::collections::HashSet;
/// Validates a user-supplied short flag and records it if free.
///
/// ```ignore
/// use std::collections::HashSet;
/// use ortho_config_macros::derive::build::validate_user_cli_short;
/// use syn::parse_quote;
///
/// let name: syn::Ident = parse_quote!(field);
/// let mut used = HashSet::new();
/// let ch = validate_user_cli_short(&name, 'f', &mut used).expect("short flag");
/// assert_eq!(ch, 'f');
/// ```
fn validate_user_cli_short(
    name: &Ident,
    user: char,
    used_shorts: &mut HashSet<char>,
) -> syn::Result<char> {
    if !user.is_ascii_alphanumeric() {
        return Err(syn::Error::new_spanned(
            name,
            format!("invalid `cli_short` '{user}': must be ASCII alphanumeric"),
        ));
    }
    if RESERVED_SHORTS.contains(&user) {
        return Err(syn::Error::new_spanned(
            name,
            format!("reserved `cli_short` '{user}' conflicts with global flags"),
        ));
    }
    if !used_shorts.insert(user) {
        return Err(syn::Error::new_spanned(name, "duplicate `cli_short` value"));
    }
    Ok(user)
}

/// Derives a default short flag from the field name.
///
/// ```ignore
/// use std::collections::HashSet;
/// use ortho_config_macros::derive::build::find_default_short_flag;
/// use syn::parse_quote;
///
/// let name: syn::Ident = parse_quote!(field);
/// let mut used = HashSet::new();
/// let ch = find_default_short_flag(&name, &mut used).expect("short flag");
/// assert_eq!(ch, 'f');
/// ```
fn find_default_short_flag(name: &Ident, used_shorts: &mut HashSet<char>) -> syn::Result<char> {
    let mut chosen: Option<char> = None;
    for ch in name.to_string().chars() {
        if !ch.is_ascii_alphanumeric() {
            continue;
        }
        for candidate in [ch.to_ascii_lowercase(), ch.to_ascii_uppercase()] {
            if !RESERVED_SHORTS.contains(&candidate) && used_shorts.insert(candidate) {
                chosen = Some(candidate);
                break;
            }
        }
        if chosen.is_some() {
            break;
        }
    }
    if let Some(c) = chosen {
        return Ok(c);
    }
    Err(syn::Error::new_spanned(
        name,
        "unable to derive a short flag; supply `cli_short` to disambiguate",
    ))
}

/// Resolves a short CLI flag ensuring uniqueness and validity.
///
/// # Examples
///
/// ```ignore
/// use std::collections::HashSet;
/// use ortho_config_macros::derive::build::resolve_short_flag;
/// use ortho_config_macros::derive::parse::FieldAttrs;
/// use syn::parse_quote;
///
/// let name: syn::Ident = parse_quote!(field);
/// let attrs = FieldAttrs::default();
/// let mut used = HashSet::new();
/// let ch = resolve_short_flag(&name, &attrs, &mut used).expect("short flag");
/// assert_eq!(ch, 'f');
/// ```
fn resolve_short_flag(
    name: &Ident,
    attrs: &FieldAttrs,
    used_shorts: &mut HashSet<char>,
) -> syn::Result<char> {
    if let Some(user) = attrs.cli_short {
        return validate_user_cli_short(name, user, used_shorts);
    }
    find_default_short_flag(name, used_shorts)
}

/// Generates fields for the defaults struct used to hold attribute-specified
/// default values.
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

/// Returns whether a long CLI flag is valid.
///
/// A valid flag is non-empty, does not start with `-`, and contains only ASCII
/// alphanumeric, hyphen or underscore characters.
///
/// # Examples
///
/// ```ignore
/// use ortho_config_macros::derive::build::is_valid_cli_long;
/// assert!(is_valid_cli_long("alpha-1"));
/// assert!(!is_valid_cli_long("-alpha"));
/// assert!(!is_valid_cli_long(""));
/// assert!(!is_valid_cli_long("bad/flag"));
/// ```
fn is_valid_cli_long(long: &str) -> bool {
    !long.is_empty()
        && !long.starts_with('-')
        && long
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Validates a long CLI flag against syntax and reserved names.
///
/// Ensures the provided flag is non-empty, uses only allowed characters and
/// does not collide with globally reserved clap flags.
///
/// # Examples
///
/// ```ignore
/// use ortho_config_macros::derive::build::validate_cli_long;
/// use syn::parse_quote;
///
/// let name: syn::Ident = parse_quote!(field);
/// validate_cli_long(&name, "alpha").expect("flag");
/// ```
fn validate_cli_long(name: &Ident, long: &str) -> syn::Result<()> {
    if !is_valid_cli_long(long) {
        return Err(syn::Error::new_spanned(
            name,
            format!(
                "invalid `cli_long` value '{long}': must be non-empty and contain only ASCII alphanumeric, '-' or '_'",
            ),
        ));
    }
    // Disallow leading '_' to avoid invalid defaults from underscored fields.
    if long.starts_with('_') {
        return Err(syn::Error::new_spanned(
            name,
            format!("invalid `cli_long` value '{long}': must not start with '_'"),
        ));
    }
    if long.starts_with('-') {
        return Err(syn::Error::new_spanned(
            name,
            format!("invalid `cli_long` value '{long}': must not start with '-'"),
        ));
    }
    if RESERVED_LONGS.contains(&long) {
        return Err(syn::Error::new_spanned(
            name,
            format!("reserved `cli_long` value '{long}': conflicts with global clap flags"),
        ));
    }
    Ok(())
}

/// Ensures no field's `cli_long` collides with the hidden `config-path` flag.
pub(crate) fn ensure_no_config_path_collision(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<()> {
    let mut used: HashSet<String> = HashSet::with_capacity(fields.len());
    for (f, attrs) in fields.iter().zip(field_attrs) {
        let name = f.ident.as_ref().expect("named field");
        let long = attrs
            .cli_long
            .clone()
            .unwrap_or_else(|| name.to_string().replace('_', "-"));
        if !used.insert(long.clone()) {
            return Err(syn::Error::new_spanned(
                name,
                format!("duplicate `cli_long` value '{long}'"),
            ));
        }
    }
    if used.contains("config-path") {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "duplicate `cli_long` value 'config-path' clashes with the hidden config flag; rename the field or specify a different `cli_long`",
        ));
    }
    Ok(())
}

/// Generates the fields for the hidden `clap::Parser` struct.
///
/// Each user field becomes `Option<T>` to record whether the CLI provided a
/// value. Long names default to the field name with underscores replaced by
/// hyphens (i.e., not fully kebab-case), so generated long flags never include
/// underscores. Short names default to the first ASCII alphanumeric character
/// of the field. These may be overridden via `cli_long` and `cli_short`
/// attributes.
pub(crate) fn build_cli_struct_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let mut used_shorts = HashSet::new();
    let mut used_longs: HashSet<String> = HashSet::with_capacity(fields.len());
    let mut result = Vec::with_capacity(fields.len());

    for (f, attrs) in fields.iter().zip(field_attrs) {
        let name = f.ident.as_ref().expect("named field");
        let ty = option_type_tokens(&f.ty);
        let long = attrs
            .cli_long
            .clone()
            .unwrap_or_else(|| name.to_string().replace('_', "-"));
        validate_cli_long(name, &long)?;
        if !used_longs.insert(long.clone()) {
            return Err(syn::Error::new_spanned(
                name,
                format!("duplicate `cli_long` value '{long}'"),
            ));
        }
        let short_ch = resolve_short_flag(name, attrs, &mut used_shorts)?;
        let long_lit = syn::LitStr::new(&long, proc_macro2::Span::call_site());
        let short_lit = syn::LitChar::new(short_ch, proc_macro2::Span::call_site());
        result.push(quote! {
            #[arg(long = #long_lit, short = #short_lit)]
            #[serde(skip_serializing_if = "Option::is_none")]
            pub #name: #ty
        });
    }

    Ok(result)
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
        quote! { ortho_config::CsvEnv::prefixed(#prefix) }
    } else {
        quote! { ortho_config::CsvEnv::raw() }
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

/// Builds discovery code for configuration files with the given extensions.
///
/// The extensions are tried sequentially; earlier entries take precedence over
/// later ones. Passing `["json", "json5", "yaml", "yml"]` will therefore
/// try `config.json` before `config.json5` and either `config.yaml` or
/// `config.yml`.
///
/// JSON and JSON5 support are only available when the `json5` feature is
/// enabled, and YAML/YML support requires the `yaml` feature.
///
/// # Examples
///
/// ```ignore
/// let tokens = build_discovery(["json", "json5", "yaml", "yml"]);
/// assert!(!tokens.is_empty());
/// ```
fn build_discovery<I, S>(exts: I) -> proc_macro2::TokenStream
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let exts = exts.into_iter().map(|s| s.as_ref().to_owned());
    quote! { try_load_config(&mut file_fig, &[#(#exts),*], &mut discovery_errors); }
}

/// Builds the XDG base directory configuration discovery snippet.
///
/// # Examples
///
/// ```ignore
/// let tokens = build_xdg_config_discovery();
/// assert!(!tokens.is_empty());
/// ```
fn build_xdg_config_discovery() -> proc_macro2::TokenStream {
    let toml = build_discovery(["toml"]);
    let json = build_discovery(["json", "json5"]);
    let yaml = build_discovery(["yaml", "yml"]);
    quote! {
        let try_load_config = |
            fig: &mut Option<ortho_config::figment::Figment>,
            exts: &[&str],
            errors: &mut Vec<ortho_config::OrthoError>,
        | {
            for ext in exts {
                let filename = format!("config.{}", ext);
                let path = match xdg_dirs.find_config_file(&filename) {
                    Some(p) => p,
                    None => continue,
                };
                match ortho_config::load_config_file(&path) {
                    Ok(new_fig) => {
                        *fig = new_fig;
                        break;
                    }
                    Err(e) => errors.push(e),
                }
            }
        };

        if file_fig.is_none() {
            #toml
        }
        #[cfg(feature = "json5")]
        if file_fig.is_none() {
            #json
        }
        #[cfg(feature = "yaml")]
        if file_fig.is_none() {
            #yaml
        }
    }
}

pub(crate) fn build_xdg_snippet(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    let prefix_lit = struct_attrs.prefix.as_deref().unwrap_or("");
    let config_discovery = build_xdg_config_discovery();
    quote! {
        #[cfg(any(unix, target_os = "redox"))]
        if file_fig.is_none() {
            let xdg_base = ortho_config::normalize_prefix(#prefix_lit);
            let xdg_dirs = if xdg_base.is_empty() {
                ortho_config::xdg::BaseDirectories::new()
            } else {
                ortho_config::xdg::BaseDirectories::with_prefix(&xdg_base)
            };
            #config_discovery
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
        let cli_figment = Figment::from(Serialized::defaults(&cli));
        #( #logic )*
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::collections::HashSet;
    use syn::{Ident, parse_quote};

    #[rstest]
    #[case("alpha")]
    #[case("alpha-1")]
    #[case("alpha_beta")]
    // Leading '-' must be rejected; clap adds them
    fn accepts_valid_long_flags(#[case] long: &str) {
        let name: Ident = parse_quote!(field);
        assert!(validate_cli_long(&name, long).is_ok());
    }

    #[rstest]
    #[case("")]
    #[case("bad/flag")]
    #[case("has space")]
    #[case("*")]
    #[case("_alpha")]
    #[case("-alpha")]
    fn rejects_invalid_long_flags(#[case] bad: &str) {
        let name: Ident = parse_quote!(field);
        let err = validate_cli_long(&name, bad).expect_err("should fail");
        assert!(err.to_string().contains("invalid `cli_long`"));
    }

    #[rstest]
    #[case("help")]
    #[case("version")]
    fn rejects_reserved_long_flags(#[case] long: &str) {
        let name: Ident = parse_quote!(field);
        let err = validate_cli_long(&name, long).expect_err("should fail");
        assert!(err.to_string().contains("reserved `cli_long` value"));
    }

    #[rstest]
    fn selects_default_lowercase() {
        let name: Ident = parse_quote!(field);
        let attrs = FieldAttrs::default();
        let mut used = HashSet::new();
        let ch = resolve_short_flag(&name, &attrs, &mut used).expect("short flag");
        assert_eq!(ch, 'f');
        assert!(used.contains(&'f'));
    }

    #[rstest]
    fn falls_back_to_uppercase() {
        let name: Ident = parse_quote!(field);
        let attrs = FieldAttrs::default();
        let mut used = HashSet::from(['f']);
        let ch = resolve_short_flag(&name, &attrs, &mut used).expect("short flag");
        assert_eq!(ch, 'F');
        assert!(used.contains(&'F'));
    }

    #[rstest]
    fn skips_leading_underscore_for_default_short() {
        let name: Ident = parse_quote!(_alpha);
        let attrs = FieldAttrs::default();
        let mut used = HashSet::new();
        let ch = resolve_short_flag(&name, &attrs, &mut used).expect("short flag");
        assert_eq!(ch, 'a');
        assert!(used.contains(&'a'));
    }

    #[rstest]
    fn errors_when_no_alphanumeric_found() {
        let name: Ident = parse_quote!(__);
        let attrs = FieldAttrs::default();
        let mut used = HashSet::new();
        let err = resolve_short_flag(&name, &attrs, &mut used).expect_err("should fail");
        assert!(err.to_string().contains("unable to derive a short flag"));
    }

    #[rstest]
    #[case('*', HashSet::new(), "invalid `cli_short`")]
    #[case('h', HashSet::new(), "reserved `cli_short`")]
    #[case(
        'f',
        HashSet::from(['f']),
        "duplicate `cli_short` value",
    )]
    fn rejects_invalid_short_flags(
        #[case] cli_short: char,
        #[case] mut used: HashSet<char>,
        #[case] expected_error: &str,
    ) {
        let name: Ident = parse_quote!(field);
        let attrs = FieldAttrs {
            cli_short: Some(cli_short),
            ..FieldAttrs::default()
        };
        let err = resolve_short_flag(&name, &attrs, &mut used).expect_err("should fail");
        assert!(err.to_string().contains(expected_error));
    }

    #[rstest]
    #[case(parse_quote! {
        struct Demo {
            #[ortho_config(cli_long = "alpha")]
            field1: u32,
            #[ortho_config(cli_long = "alpha")]
            field2: u32,
        }
    })]
    #[case(parse_quote! {
        struct Demo {
            field_one: u32,
            #[ortho_config(cli_long = "field-one")]
            field_two: u32,
        }
    })]
    // Ensure duplicates trigger a diagnostic for explicit and default-derived clashes.
    fn rejects_duplicate_long_flags_scenarios(#[case] input: syn::DeriveInput) {
        let (_, fields, _, field_attrs) =
            crate::derive::parse::parse_input(&input).expect("parse_input");
        let err = build_cli_struct_fields(&fields, &field_attrs).expect_err("should fail");
        assert!(err.to_string().contains("duplicate `cli_long` value"));
    }

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
        assert_eq!(
            ts.to_string(),
            "ortho_config :: CsvEnv :: prefixed (\"CFG_\")"
        );
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
