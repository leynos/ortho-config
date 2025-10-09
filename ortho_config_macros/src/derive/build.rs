//! Code generation helpers for the `OrthoConfig` derive macro.
//!
//! This module contains utilities shared across the code generation
//! routines. The `load_impl` submodule houses the helpers that build the
//! `load_from_iter` implementation used by the derive macro. Boolean fields are
//! handled specially with `ArgAction::SetTrue` for proper CLI flag behaviour.
//! Fields that are originally declared as `Option` emit
//! `#[serde(skip_serializing_if = "Option::is_none")]` in the generated CLI
//! struct to avoid serialising absent values.

use quote::{format_ident, quote};
use syn::{Ident, Type};

use super::parse::{FieldAttrs, StructAttrs, option_inner, vec_inner};
use heck::ToSnakeCase;
use std::collections::HashSet;

const RESERVED_SHORTS: &[char] = &['h', 'V'];
const RESERVED_LONGS: &[&str] = &["help", "version"];

#[derive(Debug)]
pub(crate) struct CliStructTokens {
    pub fields: Vec<proc_macro2::TokenStream>,
    pub used_shorts: HashSet<char>,
    pub used_longs: HashSet<String>,
}

fn option_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    if let Some(inner) = option_inner(ty) {
        quote! { Option<#inner> }
    } else {
        quote! { Option<#ty> }
    }
}

fn is_bool_type(ty: &Type) -> bool {
    fn matches_bool(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Path(type_path)
                if type_path.qself.is_none() && type_path.path.is_ident("bool")
        )
    }

    matches_bool(ty) || option_inner(ty).is_some_and(matches_bool)
}

/// Resolves a short CLI flag ensuring uniqueness and validity.
///
/// # Examples
///
/// Validates a user-supplied short flag and records it if free.
///
/// ```ignore
/// use std::collections::HashSet;
/// use ortho_config_macros::derive::build::validate_user_cli_short;
/// use syn::parse_quote;
///
/// let name: syn::Ident = parse_quote!(field);
/// let mut used = HashSet::new();
/// let ch = validate_user_cli_short(&name, 'f', &used).expect("short flag");
/// used.insert(ch);
/// assert_eq!(ch, 'f');
/// ```
fn validate_user_cli_short(
    name: &Ident,
    user: char,
    used_shorts: &HashSet<char>,
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
    if used_shorts.contains(&user) {
        return Err(syn::Error::new_spanned(name, "duplicate `cli_short` value"));
    }
    Ok(user)
}

/// Returns `true` if the candidate short flag is free for use.
///
/// ```ignore
/// use std::collections::HashSet;
/// use ortho_config_macros::derive::build::is_short_flag_available;
///
/// let used: HashSet<char> = ['a'].into_iter().collect();
/// assert!(!is_short_flag_available('a', &used));
/// assert!(is_short_flag_available('b', &used));
/// ```
fn is_short_flag_available(candidate: char, used_shorts: &HashSet<char>) -> bool {
    !RESERVED_SHORTS.contains(&candidate) && !used_shorts.contains(&candidate)
}

/// Generates lowercase and uppercase variants of the provided character.
///
/// ```ignore
/// use ortho_config_macros::derive::build::generate_short_flag_candidates;
///
/// assert_eq!(generate_short_flag_candidates('a'), ['a', 'A']);
/// assert_eq!(generate_short_flag_candidates('7'), ['7', '7']);
/// ```
fn generate_short_flag_candidates(ch: char) -> [char; 2] {
    [ch.to_ascii_lowercase(), ch.to_ascii_uppercase()]
}

/// Attempts to claim the first available short flag from the candidates.
///
/// ```ignore
/// use std::collections::HashSet;
/// use ortho_config_macros::derive::build::try_claim_short_flag;
///
/// let mut used = HashSet::new();
/// let claimed = try_claim_short_flag(['a', 'A'], &mut used);
/// assert_eq!(claimed, Some('a'));
/// ```
fn try_claim_short_flag(candidates: [char; 2], used_shorts: &mut HashSet<char>) -> Option<char> {
    for candidate in candidates {
        if is_short_flag_available(candidate, used_shorts) {
            used_shorts.insert(candidate);
            return Some(candidate);
        }
    }
    None
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
    for ch in name.to_string().chars() {
        if !ch.is_ascii_alphanumeric() {
            continue;
        }
        let candidates = generate_short_flag_candidates(ch);
        if let Some(c) = try_claim_short_flag(candidates, used_shorts) {
            return Ok(c);
        }
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
        let claimed = validate_user_cli_short(name, user, used_shorts)?;
        used_shorts.insert(claimed);
        return Ok(claimed);
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
/// alphanumeric characters or hyphens. Underscores are rejected to keep the CLI
/// syntax aligned with the documentation.
///
/// Allows only ASCII alphanumeric characters or hyphens and rejects
/// underscores to keep long flags consistent with the user guide.
///
/// # Examples
///
/// ```ignore
/// use ortho_config_macros::derive::build::is_valid_cli_long;
/// assert!(is_valid_cli_long("alpha-1"));
/// assert!(!is_valid_cli_long("-alpha"));
/// assert!(!is_valid_cli_long(""));
/// assert!(!is_valid_cli_long("bad/flag"));
/// assert!(!is_valid_cli_long("with_underscore"));
/// ```
fn is_valid_cli_long(long: &str) -> bool {
    !long.is_empty()
        && !long.starts_with('-')
        && long.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
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
                "invalid `cli_long` value '{long}': must be non-empty and contain only ASCII alphanumeric or '-'",
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

/// Generates the fields for the hidden `clap::Parser` struct.
///
/// Each user field becomes `Option<T>` to record whether the CLI provided a
/// value. Long names default to the field name with underscores replaced by
/// hyphens, so generated long flags never include
/// underscores. Short names default to the first ASCII alphanumeric character
/// of the field. These may be overridden via `cli_long` and `cli_short`
/// attributes.
pub(crate) fn build_cli_struct_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<CliStructTokens> {
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
        let is_bool = is_bool_type(&f.ty);
        let serde_attr = option_inner(&f.ty)
            .map(|_| quote! { #[serde(skip_serializing_if = "Option::is_none")] });
        let arg_attr = if is_bool {
            quote! {
                #[arg(long = #long_lit, short = #short_lit, action = clap::ArgAction::SetTrue)]
            }
        } else {
            quote! {
                #[arg(long = #long_lit, short = #short_lit)]
            }
        };
        result.push(quote! {
            #arg_attr
            #serde_attr
            pub #name: #ty
        });
    }

    Ok(CliStructTokens {
        fields: result,
        used_shorts,
        used_longs,
    })
}

pub(crate) fn build_config_flag_field(
    struct_attrs: &StructAttrs,
    used_shorts: &HashSet<char>,
    used_longs: &HashSet<String>,
) -> syn::Result<proc_macro2::TokenStream> {
    let name = Ident::new("config_path", proc_macro2::Span::call_site());
    let discovery = struct_attrs.discovery.as_ref();
    let long = discovery
        .and_then(|attrs| attrs.config_cli_long.clone())
        .unwrap_or_else(|| String::from("config-path"));
    validate_cli_long(&name, &long)?;
    if used_longs.contains(&long) {
        return Err(syn::Error::new_spanned(
            &name,
            format!("duplicate `cli_long` value '{long}' conflicts with the generated config flag"),
        ));
    }
    let long_lit = syn::LitStr::new(&long, proc_macro2::Span::call_site());
    let mut arg_meta: Vec<proc_macro2::TokenStream> = vec![quote! { long = #long_lit }];
    if let Some(short) = discovery.and_then(|attrs| attrs.config_cli_short) {
        let claimed = validate_user_cli_short(&name, short, used_shorts)?;
        let short_lit = syn::LitChar::new(claimed, proc_macro2::Span::call_site());
        arg_meta.push(quote! { short = #short_lit });
    }
    let visible = discovery
        .and_then(|attrs| attrs.config_cli_visible)
        .unwrap_or(false);
    if !visible {
        arg_meta.push(quote! { hide = true });
    }
    arg_meta.push(quote! { value_name = "PATH" });
    if visible {
        arg_meta.push(quote! { help = "Path to the configuration file" });
    }
    let serde_attr = quote! { #[serde(skip_serializing_if = "Option::is_none")] };
    Ok(quote! {
        #[arg( #( #arg_meta ),* )]
        #serde_attr
        pub config_path: Option<std::path::PathBuf>
    })
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

pub(crate) fn compute_config_env_var(struct_attrs: &StructAttrs) -> String {
    struct_attrs.prefix.as_deref().map_or_else(
        || String::from("CONFIG_PATH"),
        |prefix| format!("{prefix}CONFIG_PATH"),
    )
}

pub(crate) fn build_config_env_var(struct_attrs: &StructAttrs) -> proc_macro2::TokenStream {
    let var = compute_config_env_var(struct_attrs);
    quote! { #var }
}

pub(crate) fn compute_dotfile_name(struct_attrs: &StructAttrs) -> String {
    if let Some(prefix) = &struct_attrs.prefix {
        let base = prefix.trim_end_matches('_').to_ascii_lowercase();
        format!(".{base}.toml")
    } else {
        String::from(".config.toml")
    }
}

pub(crate) fn default_app_name(struct_attrs: &StructAttrs, ident: &Ident) -> String {
    if let Some(prefix) = &struct_attrs.prefix {
        let normalised = prefix.trim_end_matches('_').to_ascii_lowercase();
        if !normalised.is_empty() {
            return normalised;
        }
    }
    ident.to_string().to_snake_case()
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
    use crate::derive::parse::DiscoveryAttrs;
    use ortho_config::figment::{Figment, providers::Serialized};
    use rstest::rstest;
    use std::collections::HashSet;
    use syn::{Ident, parse_quote};

    #[rstest]
    #[case("alpha")]
    #[case("alpha-1")]
    // Leading '-' must be rejected; clap adds them
    fn accepts_valid_long_flags(#[case] long: &str) {
        let name: Ident = parse_quote!(field);
        assert!(validate_cli_long(&name, long).is_ok());
    }

    #[rstest]
    #[case("")]
    #[case("bad/flag")]
    #[case("alpha_beta")]
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
    #[case::long(
        parse_quote! {
            struct Demo {
                #[ortho_config(cli_long = "config")]
                value: u32,
            }
        },
        DiscoveryAttrs {
            config_cli_long: Some(String::from("config")),
            ..DiscoveryAttrs::default()
        },
        "duplicate `cli_long` value",
    )]
    #[case::short(
        parse_quote! {
            struct Demo {
                value: u32,
            }
        },
        DiscoveryAttrs {
            config_cli_short: Some('v'),
            ..DiscoveryAttrs::default()
        },
        "duplicate `cli_short` value",
    )]
    fn config_flag_rejects_duplicate_from_fields(
        #[case] input: syn::DeriveInput,
        #[case] discovery_attrs: DiscoveryAttrs,
        #[case] expected_error: &str,
    ) {
        let (_, fields, mut struct_attrs, field_attrs) =
            crate::derive::parse::parse_input(&input).expect("parse_input");
        let cli = build_cli_struct_fields(&fields, &field_attrs).expect("build cli fields");
        struct_attrs.discovery = Some(discovery_attrs);
        let err = build_config_flag_field(&struct_attrs, &cli.used_shorts, &cli.used_longs)
            .expect_err("should fail");
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

    #[test]
    fn bool_fields_do_not_emit_skip_serializing_if() {
        // Mirror the generated CLI field to confirm Figment receives no value when the flag is absent.
        #[derive(serde::Serialize)]
        struct __Cli {
            excited: Option<bool>,
        }

        let input: syn::DeriveInput = parse_quote! {
            struct Demo {
                excited: bool,
            }
        };
        let (_, fields, _, field_attrs) =
            crate::derive::parse::parse_input(&input).expect("parse_input");
        let tokens = build_cli_struct_fields(&fields, &field_attrs).expect("build cli fields");
        let field_ts = tokens
            .fields
            .first()
            .expect("generated field tokens")
            .to_string();
        assert!(
            field_ts.contains("ArgAction :: SetTrue"),
            "boolean CLI fields should use ArgAction::SetTrue"
        );
        assert!(
            !field_ts.contains("skip_serializing_if"),
            "boolean CLI fields should not emit skip_serializing_if"
        );

        let cli = __Cli { excited: None };
        let figment = Figment::from(Serialized::defaults(&cli));
        assert!(
            figment.extract_inner::<bool>("excited").is_err(),
            "Absent boolean flags should not appear in Figment"
        );
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
