//! Tests for CLI flag validation and builders.

use super::cli_flags::resolve_short_flag;
use super::{build_cli_struct_fields, validate_cli_long, validate_user_cli_short};
use crate::derive::parse::FieldAttrs;
use anyhow::{Result, anyhow, ensure};
use rstest::rstest;
use std::collections::HashSet;
use syn::Ident;

#[rstest]
#[case("alpha")]
#[case("alpha-1")]
fn accepts_valid_long_flags(#[case] long: &str) -> Result<()> {
    let name: Ident = syn::parse_quote!(field);
    validate_cli_long(&name, long).map_err(|err| anyhow!(err))?;
    Ok(())
}

#[test]
fn skips_fields_marked_with_skip_cli() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct Demo {
            #[ortho_config(skip_cli)]
            values: std::collections::BTreeMap<String, String>,
        }
    };
    let (_, fields, _, field_attrs) = crate::derive::parse::parse_input(&input)?;
    let tokens = build_cli_struct_fields(&fields, &field_attrs)?;
    ensure!(
        tokens.fields.is_empty(),
        "expected CLI struct to omit skipped field"
    );
    Ok(())
}

/// Renders the generated `#[arg(...)]` attribute for a single-field struct.
fn generated_attribute(ty: &str) -> Result<String> {
    let input: syn::DeriveInput = syn::parse_str(&format!("struct Demo {{ excited: {ty} }}"))
        .map_err(|err| anyhow!(err))?;
    let (_, fields, _, field_attrs) = crate::derive::parse::parse_input(&input)?;
    let tokens = build_cli_struct_fields(&fields, &field_attrs)?;
    tokens
        .fields
        .first()
        .map(std::string::ToString::to_string)
        .ok_or_else(|| anyhow!("expected generated field tokens"))
}

#[rstest]
#[case("bool")]
#[case("Option<bool>")]
fn boolean_fields_accept_an_optional_value(#[case] ty: &str) -> Result<()> {
    let field_ts = generated_attribute(ty)?;
    for expected in [
        "num_args = 0 ..= 1",
        "require_equals = true",
        "default_missing_value = \"true\"",
    ] {
        ensure!(
            field_ts.contains(expected),
            "{ty} fields should emit `{expected}`, got: {field_ts}"
        );
    }
    ensure!(
        !field_ts.contains("ArgAction :: SetTrue"),
        "{ty} fields should not use presence-only ArgAction::SetTrue, got: {field_ts}"
    );
    Ok(())
}

#[rstest]
#[case("std::string::String")]
#[case("Option<u16>")]
fn non_boolean_fields_take_a_value(#[case] ty: &str) -> Result<()> {
    let field_ts = generated_attribute(ty)?;
    ensure!(
        !field_ts.contains("num_args") && !field_ts.contains("default_missing_value"),
        "{ty} fields should keep the default clap value semantics, got: {field_ts}"
    );
    Ok(())
}

#[rstest]
#[case("")]
#[case("bad/flag")]
#[case("alpha_beta")]
#[case("has space")]
#[case("*")]
#[case("_alpha")]
#[case("-alpha")]
fn rejects_invalid_long_flags(#[case] bad: &str) -> Result<()> {
    let name: Ident = syn::parse_quote!(field);
    let Err(err) = validate_cli_long(&name, bad) else {
        return Err(anyhow!("expected invalid long flag for {bad}"));
    };
    ensure!(
        err.to_string().contains("invalid `cli_long`"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[rstest]
#[case("help")]
#[case("version")]
fn rejects_reserved_long_flags(#[case] long: &str) -> Result<()> {
    let name: Ident = syn::parse_quote!(field);
    let Err(err) = validate_cli_long(&name, long) else {
        return Err(anyhow!("expected reserved long flag error for {long}"));
    };
    ensure!(
        err.to_string().contains("reserved `cli_long`"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[rstest]
fn selects_default_lowercase() -> Result<()> {
    let name: Ident = syn::parse_quote!(field);
    let attrs = FieldAttrs::default();
    let mut used = HashSet::new();
    let ch = resolve_short_flag(&name, &attrs, &mut used).map_err(|err| anyhow!(err))?;
    ensure!(ch == 'f', "expected 'f', got {ch}");
    ensure!(used.contains(&'f'), "expected 'f' to be recorded");
    Ok(())
}

#[rstest]
fn falls_back_to_uppercase() -> Result<()> {
    let name: Ident = syn::parse_quote!(field);
    let attrs = FieldAttrs::default();
    let mut used = HashSet::from(['f']);
    let ch = resolve_short_flag(&name, &attrs, &mut used).map_err(|err| anyhow!(err))?;
    ensure!(ch == 'F', "expected 'F', got {ch}");
    ensure!(used.contains(&'F'), "expected 'F' to be recorded");
    Ok(())
}

#[rstest]
fn skips_leading_underscore_for_default_short() -> Result<()> {
    let name: Ident = syn::parse_quote!(_alpha);
    let attrs = FieldAttrs::default();
    let mut used = HashSet::new();
    let ch = resolve_short_flag(&name, &attrs, &mut used).map_err(|err| anyhow!(err))?;
    ensure!(ch == 'a', "expected 'a', got {ch}");
    ensure!(used.contains(&'a'), "expected 'a' to be recorded");
    Ok(())
}

#[rstest]
fn errors_when_no_alphanumeric_found() -> Result<()> {
    let name: Ident = syn::parse_quote!(__);
    let attrs = FieldAttrs::default();
    let mut used = HashSet::new();
    match resolve_short_flag(&name, &attrs, &mut used) {
        Ok(_) => Err(anyhow!("expected failure deriving short flag")),
        Err(err) => {
            ensure!(
                err.to_string().contains("unable to derive a short flag"),
                "unexpected error: {err}"
            );
            Ok(())
        }
    }
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
) -> Result<()> {
    let name: Ident = syn::parse_quote!(field);
    let attrs = FieldAttrs {
        cli_short: Some(cli_short),
        ..FieldAttrs::default()
    };
    match resolve_short_flag(&name, &attrs, &mut used) {
        Ok(_) => Err(anyhow!("expected short flag error")),
        Err(err) => {
            ensure!(
                err.to_string().contains(expected_error),
                "unexpected error: {err}"
            );
            Ok(())
        }
    }
}

#[rstest]
#[case('a', HashSet::new(), Some('a'), None)]
#[case('*', HashSet::new(), None, Some("invalid `cli_short`"))]
#[case('h', HashSet::new(), None, Some("reserved `cli_short`"))]
#[case('f', HashSet::from(['f']), None, Some("duplicate `cli_short` value"))]
fn validates_user_short_flags(
    #[case] cli_short: char,
    #[case] used: HashSet<char>,
    #[case] expected_ok: Option<char>,
    #[case] expected_err: Option<&str>,
) -> Result<()> {
    let name: Ident = syn::parse_quote!(field);
    match validate_user_cli_short(&name, cli_short, &used) {
        Ok(ch) => {
            let Some(expected) = expected_ok else {
                return Err(anyhow!("expected short flag error"));
            };
            ensure!(ch == expected, "expected {expected}, got {ch}");
        }
        Err(err) => {
            let Some(expected) = expected_err else {
                return Err(anyhow!("expected short flag to be accepted"));
            };
            ensure!(
                err.to_string().contains(expected),
                "unexpected error: {err}"
            );
        }
    }
    Ok(())
}

#[test]
fn rejects_mismatched_field_metadata_lengths() -> Result<()> {
    let fields: Vec<syn::Field> = vec![syn::parse_quote!(pub alpha: bool)];
    let mut attrs = vec![FieldAttrs::default(); 2];
    attrs
        .first_mut()
        .ok_or_else(|| anyhow!("expected at least one attribute entry"))?
        .cli_long = Some("alpha".into());
    match build_cli_struct_fields(&fields, &attrs) {
        Ok(_) => Err(anyhow!("expected CLI field metadata mismatch")),
        Err(err) => {
            ensure!(
                err.to_string().contains("CLI field metadata mismatch"),
                "unexpected error: {err}"
            );
            Ok(())
        }
    }
}
