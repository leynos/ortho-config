//! CLI helper builders used by the `OrthoConfig` derive macro.
//!
//! This module encapsulates generation and validation of CLI flag metadata.
//! It keeps track of claimed short and long flags to avoid collisions and
//! surfaces ergonomic diagnostics when callers need to supply overrides.

use std::collections::HashSet;

use heck::ToKebabCase;
use quote::{quote, quote_spanned};
use syn::{Ident, Type};

use crate::derive::parse::{FieldAttrs, option_inner};

const RESERVED_SHORTS: &[char] = &['h', 'V'];
const RESERVED_LONGS: &[&str] = &["help", "version"];

#[derive(Debug)]
pub(crate) struct CliStructTokens {
    pub fields: Vec<proc_macro2::TokenStream>,
    pub used_shorts: HashSet<char>,
    pub used_longs: HashSet<String>,
    pub field_names: HashSet<String>,
}

pub(super) fn option_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
    option_inner(ty).map_or_else(|| quote! { Option<#ty> }, |inner| quote! { Option<#inner> })
}

fn is_bool_type(ty: &Type) -> bool {
    let inner = option_inner(ty).unwrap_or(ty);
    matches!(
        inner,
        Type::Path(type_path) if type_path.qself.is_none() && type_path.path.is_ident("bool")
    )
}

fn is_empty_long(long: &str) -> bool {
    long.is_empty()
}

fn has_invalid_prefix(long: &str) -> bool {
    long.starts_with(['-', '_'])
}

fn has_invalid_chars(long: &str) -> bool {
    !long.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn invalid_prefix_message(long: &str) -> Option<String> {
    if !has_invalid_prefix(long) {
        return None;
    }
    let prefix = if long.starts_with('-') { '-' } else { '_' };
    Some(format!(
        "invalid `cli_long` '{long}': must not start with '{prefix}'"
    ))
}

fn long_validation_error(long: &str) -> Option<String> {
    if is_empty_long(long) {
        Some(format!("invalid `cli_long` '{long}': must be non-empty"))
    } else if let Some(message) = invalid_prefix_message(long) {
        Some(message)
    } else if has_invalid_chars(long) {
        Some(format!(
            "invalid `cli_long` '{long}': must contain only ASCII alphanumeric characters or '-'"
        ))
    } else {
        None
    }
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
pub(super) fn validate_user_cli_short(
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

pub(super) fn resolve_short_flag(
    name: &Ident,
    attrs: &FieldAttrs,
    used_shorts: &mut HashSet<char>,
) -> syn::Result<char> {
    if let Some(user) = attrs.cli_short {
        let ch = validate_user_cli_short(name, user, used_shorts)?;
        used_shorts.insert(ch);
        return Ok(ch);
    }

    let derived = name
        .to_string()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(|c| [c.to_ascii_lowercase(), c.to_ascii_uppercase()])
        .find(|candidate| !RESERVED_SHORTS.contains(candidate) && !used_shorts.contains(candidate))
        .ok_or_else(|| {
            syn::Error::new_spanned(
                name,
                "unable to derive a short flag; supply `cli_short` to disambiguate",
            )
        })?;
    used_shorts.insert(derived);
    Ok(derived)
}

pub(super) fn validate_cli_long(name: &Ident, long: &str) -> syn::Result<()> {
    if let Some(message) = long_validation_error(long) {
        return Err(syn::Error::new_spanned(name, message));
    }
    if RESERVED_LONGS.contains(&long) {
        return Err(syn::Error::new_spanned(
            name,
            format!("reserved `cli_long` '{long}' conflicts with global clap flags"),
        ));
    }
    Ok(())
}

/// Context for tracking used CLI flags and field names during field processing.
struct CliFieldContext {
    used_shorts: HashSet<char>,
    used_longs: HashSet<String>,
    field_names: HashSet<String>,
}

impl CliFieldContext {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            used_shorts: HashSet::new(),
            used_longs: HashSet::with_capacity(capacity),
            field_names: HashSet::with_capacity(capacity),
        }
    }
}

fn process_cli_field(
    field: &syn::Field,
    attrs: &FieldAttrs,
    context: &mut CliFieldContext,
) -> syn::Result<proc_macro2::TokenStream> {
    let Some(name) = field.ident.as_ref() else {
        return Err(syn::Error::new_spanned(
            field,
            "unnamed (tuple) fields are not supported for CLI derive",
        ));
    };

    let ty = option_type_tokens(&field.ty);
    let field_name = name.to_string();
    context.field_names.insert(field_name.clone());

    let long = attrs
        .cli_long
        .clone()
        .unwrap_or_else(|| field_name.to_kebab_case());
    validate_cli_long(name, &long)?;

    if !context.used_longs.insert(long.clone()) {
        return Err(syn::Error::new_spanned(
            name,
            format!("duplicate `cli_long` value '{long}'"),
        ));
    }

    let short_ch = resolve_short_flag(name, attrs, &mut context.used_shorts)?;
    let long_lit = syn::LitStr::new(&long, proc_macro2::Span::call_site());
    let short_lit = syn::LitChar::new(short_ch, proc_macro2::Span::call_site());
    let is_bool = is_bool_type(&field.ty);
    let span = name.span();

    let arg_attr = if is_bool {
        quote_spanned! { span =>
            #[arg(long = #long_lit, short = #short_lit, action = clap::ArgAction::SetTrue)]
        }
    } else {
        quote_spanned! { span =>
            #[arg(long = #long_lit, short = #short_lit)]
        }
    };

    let serde_attr = if is_bool {
        proc_macro2::TokenStream::new()
    } else {
        quote_spanned! { span =>
            #[serde(skip_serializing_if = "Option::is_none")]
        }
    };

    Ok(quote_spanned! { span =>
        #arg_attr
        #serde_attr
        pub #name: #ty
    })
}

pub(crate) fn build_cli_struct_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> syn::Result<CliStructTokens> {
    if fields.len() != field_attrs.len() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "CLI field metadata mismatch: expected {} `FieldAttrs` entries but found {}; lengths must match to avoid silently dropping metadata",
                fields.len(),
                field_attrs.len()
            ),
        ));
    }

    let mut context = CliFieldContext::with_capacity(fields.len());
    let mut result = Vec::with_capacity(fields.len());

    for (field, attrs) in fields.iter().zip(field_attrs) {
        let field_tokens = process_cli_field(field, attrs, &mut context)?;
        result.push(field_tokens);
    }

    let CliFieldContext {
        used_shorts,
        used_longs,
        field_names,
    } = context;

    Ok(CliStructTokens {
        fields: result,
        used_shorts,
        used_longs,
        field_names,
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        unfulfilled_lint_expectations,
        reason = "clippy::expect_used is denied globally; tests may not hit those branches"
    )]
    #![expect(
        clippy::expect_used,
        reason = "tests panic to surface configuration mistakes"
    )]
    use super::*;
    use crate::derive::parse::FieldAttrs;
    use rstest::rstest;

    #[rstest]
    #[case("alpha")]
    #[case("alpha-1")]
    fn accepts_valid_long_flags(#[case] long: &str) {
        let name: Ident = syn::parse_quote!(field);
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
        let name: Ident = syn::parse_quote!(field);
        let err = validate_cli_long(&name, bad).expect_err("should fail");
        assert!(err.to_string().contains("invalid `cli_long`"));
    }

    #[rstest]
    #[case("help")]
    #[case("version")]
    fn rejects_reserved_long_flags(#[case] long: &str) {
        let name: Ident = syn::parse_quote!(field);
        let err = validate_cli_long(&name, long).expect_err("should fail");
        assert!(err.to_string().contains("reserved `cli_long`"));
    }

    #[rstest]
    fn selects_default_lowercase() {
        let name: Ident = syn::parse_quote!(field);
        let attrs = FieldAttrs::default();
        let mut used = HashSet::new();
        let ch = resolve_short_flag(&name, &attrs, &mut used).expect("short flag");
        assert_eq!(ch, 'f');
        assert!(used.contains(&'f'));
    }

    #[rstest]
    fn falls_back_to_uppercase() {
        let name: Ident = syn::parse_quote!(field);
        let attrs = FieldAttrs::default();
        let mut used = HashSet::from(['f']);
        let ch = resolve_short_flag(&name, &attrs, &mut used).expect("short flag");
        assert_eq!(ch, 'F');
        assert!(used.contains(&'F'));
    }

    #[rstest]
    fn skips_leading_underscore_for_default_short() {
        let name: Ident = syn::parse_quote!(_alpha);
        let attrs = FieldAttrs::default();
        let mut used = HashSet::new();
        let ch = resolve_short_flag(&name, &attrs, &mut used).expect("short flag");
        assert_eq!(ch, 'a');
        assert!(used.contains(&'a'));
    }

    #[rstest]
    fn errors_when_no_alphanumeric_found() {
        let name: Ident = syn::parse_quote!(__);
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
        let name: Ident = syn::parse_quote!(field);
        let attrs = FieldAttrs {
            cli_short: Some(cli_short),
            ..FieldAttrs::default()
        };
        let err = resolve_short_flag(&name, &attrs, &mut used).expect_err("should fail");
        assert!(err.to_string().contains(expected_error));
    }

    #[test]
    fn rejects_mismatched_field_metadata_lengths() {
        let fields: Vec<syn::Field> = vec![syn::parse_quote!(pub alpha: bool)];
        let mut attrs = vec![FieldAttrs::default(); 2];
        if let Some(first) = attrs.first_mut() {
            first.cli_long = Some("alpha".into());
        } else {
            panic!("expected at least one attribute entry");
        }
        let err = build_cli_struct_fields(&fields, &attrs).expect_err("should fail");
        assert!(err.to_string().contains("CLI field metadata mismatch"));
    }
}
