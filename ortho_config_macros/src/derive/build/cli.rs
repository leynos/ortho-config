use std::collections::HashSet;

use quote::quote;
use syn::{Ident, Type};

use crate::derive::parse::{FieldAttrs, option_inner};

const RESERVED_SHORTS: &[char] = &['h', 'V'];
const RESERVED_LONGS: &[&str] = &["help", "version"];

#[derive(Debug)]
pub(crate) struct CliStructTokens {
    pub fields: Vec<proc_macro2::TokenStream>,
    pub used_shorts: HashSet<char>,
    pub used_longs: HashSet<String>,
}

pub(super) fn option_type_tokens(ty: &Type) -> proc_macro2::TokenStream {
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

fn is_short_flag_available(candidate: char, used_shorts: &HashSet<char>) -> bool {
    !RESERVED_SHORTS.contains(&candidate) && !used_shorts.contains(&candidate)
}

fn generate_short_flag_candidates(ch: char) -> [char; 2] {
    [ch.to_ascii_lowercase(), ch.to_ascii_uppercase()]
}

fn try_claim_short_flag(candidates: [char; 2], used_shorts: &mut HashSet<char>) -> Option<char> {
    for candidate in candidates {
        if is_short_flag_available(candidate, used_shorts) {
            used_shorts.insert(candidate);
            return Some(candidate);
        }
    }
    None
}

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

pub(super) fn resolve_short_flag(
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

fn is_valid_cli_long(long: &str) -> bool {
    !long.is_empty()
        && !long.starts_with('-')
        && long.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

pub(super) fn validate_cli_long(name: &Ident, long: &str) -> syn::Result<()> {
    if !is_valid_cli_long(long) {
        return Err(syn::Error::new_spanned(
            name,
            format!(
                "invalid `cli_long` value '{long}': must be non-empty and contain only ASCII alphanumeric or '-'",
            ),
        ));
    }
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

#[cfg(test)]
mod tests {
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
        assert!(err.to_string().contains("reserved `cli_long` value"));
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
}
