//! Tests for clap attribute parsing helpers.

use super::super::parse_input;
use crate::derive::parse::{
    ClapInferredDefault, FieldAttrs, clap_field_is_subcommand, clap_variant_name,
};
use anyhow::{Result, anyhow, ensure};
use quote::ToTokens;
use syn::{DeriveInput, parse_quote};

fn expr_tokens(expr: &syn::Expr) -> String {
    expr.to_token_stream().to_string()
}

fn type_tokens(ty: &syn::Type) -> String {
    ty.to_token_stream().to_string()
}

/// Parses a `DeriveInput` and returns the [`FieldAttrs`] for the first field.
fn parse_first_field_attrs(input: &DeriveInput) -> Result<FieldAttrs> {
    let (_, _, _, attrs_vec) = parse_input(input).map_err(|err| anyhow!(err))?;
    attrs_vec
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("missing field attributes"))
}

/// Helper to parse input and extract the inferred default from the first field.
fn parse_and_extract_default(input: &DeriveInput) -> Result<syn::Expr> {
    let attrs = parse_first_field_attrs(input)?;
    let Some(inferred) = attrs.inferred_clap_default.as_ref() else {
        return Err(anyhow!("missing inferred default"));
    };
    let expr = match inferred {
        ClapInferredDefault::Value(default) => &default.value,
        ClapInferredDefault::ValueT(expr) | ClapInferredDefault::ValuesT(expr) => expr,
    };
    Ok(expr.clone())
}

/// Helper to assert that generated expression tokens contain expected
/// substrings.
fn assert_tokens_contain(expr: &syn::Expr, expected_substrings: &[&str]) -> Result<()> {
    let tokens = expr_tokens(expr);
    for expected in expected_substrings {
        ensure!(
            tokens.contains(expected),
            "expected token substring '{expected}' in generated expression: {tokens}",
        );
    }
    Ok(())
}

fn first_variant(input: &DeriveInput) -> Result<&syn::Variant> {
    let syn::Data::Enum(data) = &input.data else {
        return Err(anyhow!("expected enum"));
    };
    data.variants
        .first()
        .ok_or_else(|| anyhow!("missing first variant"))
}

fn first_field(input: &DeriveInput) -> Result<&syn::Field> {
    let syn::Data::Struct(data) = &input.data else {
        return Err(anyhow!("expected struct"));
    };
    data.fields
        .iter()
        .next()
        .ok_or_else(|| anyhow!("missing first field"))
}

#[test]
fn infers_default_from_clap_default_value_t_when_requested() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value_t = String::from("!"))]
            #[ortho_config(cli_default_as_absent)]
            punctuation: String,
        }
    };

    let inferred = parse_and_extract_default(&input)?;

    let expected: syn::Expr = parse_quote! {
        String::from("!")
    };
    ensure!(
        expr_tokens(&inferred) == expr_tokens(&expected),
        "expected inferred default {}, got {}",
        expr_tokens(&expected),
        expr_tokens(&inferred),
    );
    Ok(())
}

#[test]
fn infers_default_from_clap_default_values_t_when_requested() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_values_t = ["a", "b"])]
            #[ortho_config(cli_default_as_absent)]
            values: Vec<String>,
        }
    };

    let inferred = parse_and_extract_default(&input)?;
    assert_tokens_contain(&inferred, &["\"a\"", "\"b\""])?;
    Ok(())
}

#[test]
fn captures_default_value_parser_metadata_when_requested() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value = "42", value_parser = parse_number)]
            #[ortho_config(cli_default_as_absent)]
            answer: u32,
        }
    };

    let attrs = parse_first_field_attrs(&input)?;
    let Some(ClapInferredDefault::Value(default)) = attrs.inferred_clap_default else {
        return Err(anyhow!("expected inferred default_value metadata"));
    };
    ensure!(
        expr_tokens(&default.value) == "\"42\"",
        "expected raw default expression, got {}",
        expr_tokens(&default.value),
    );
    ensure!(
        default
            .value_parser
            .as_ref()
            .is_some_and(|parser| expr_tokens(parser) == "parse_number"),
        "expected custom value parser metadata",
    );
    ensure!(
        matches!(
            default.shape,
            super::super::type_utils::ClapDefaultValueShape::Scalar
        ),
        "expected scalar field shape",
    );
    Ok(())
}

#[test]
fn captures_value_enum_marker_for_default_value() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value = "fast", value_enum)]
            #[ortho_config(cli_default_as_absent)]
            mode: Mode,
        }
    };

    let attrs = parse_first_field_attrs(&input)?;
    let Some(ClapInferredDefault::Value(default)) = attrs.inferred_clap_default else {
        return Err(anyhow!("expected inferred default_value metadata"));
    };
    ensure!(default.value_enum, "expected value_enum marker");
    ensure!(
        type_tokens(&default.leaf_type) == "Mode",
        "expected leaf type Mode, got {}",
        type_tokens(&default.leaf_type),
    );
    Ok(())
}

#[test]
fn rejects_nested_default_value_shape() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value = "value")]
            #[ortho_config(cli_default_as_absent)]
            values: Option<Vec<String>>,
        }
    };

    let err = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow!("expected unsupported default_value shape error"))?;
    let err_text = err.to_string();
    ensure!(
        err_text.contains("does not support nested `Option` and `Vec` wrappers"),
        "expected unsupported-shape diagnostic, got {err_text}",
    );
    ensure!(
        err_text.contains("use a scalar, `Option<T>`, or `Vec<T>`"),
        "expected supported alternatives, got {err_text}",
    );
    Ok(())
}

#[test]
fn rejects_map_default_value_shape() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value = "value")]
            #[ortho_config(cli_default_as_absent)]
            values: std::collections::BTreeMap<String, String>,
        }
    };

    let err = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow!("expected unsupported default_value shape error"))?;
    ensure!(
        err.to_string().contains("does not support map fields"),
        "expected map-shape diagnostic, got {err}",
    );
    Ok(())
}

#[test]
fn does_not_infer_default_without_cli_default_as_absent() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value_t = String::from("!"))]
            punctuation: String,
        }
    };

    let attrs = parse_first_field_attrs(&input)?;
    ensure!(
        attrs.default.is_none(),
        "default should not be inferred unless cli_default_as_absent is set",
    );
    Ok(())
}

#[test]
fn explicit_ortho_default_takes_precedence_over_inferred_clap_default() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value_t = String::from("clap"))]
            #[ortho_config(default = String::from("ortho"), cli_default_as_absent)]
            punctuation: String,
        }
    };

    let attrs = parse_first_field_attrs(&input)?;
    let default_expr = attrs
        .default
        .as_ref()
        .ok_or_else(|| anyhow!("missing field default"))?;
    let expected: syn::Expr = parse_quote! { String::from("ortho") };
    ensure!(
        expr_tokens(default_expr) == expr_tokens(&expected),
        "explicit ortho default should win over clap inference",
    );
    Ok(())
}

#[test]
fn parenthesised_clap_attributes_are_consumed_without_error() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(long, num_args(2), default_value_t = 5)]
            #[ortho_config(cli_default_as_absent)]
            count: u32,
        }
    };

    let inferred = parse_and_extract_default(&input)?;
    ensure!(
        expr_tokens(&inferred) == "5",
        "expected inferred default 5, got {}",
        expr_tokens(&inferred),
    );
    Ok(())
}

#[test]
fn duplicate_clap_defaults_are_rejected() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value_t = 1, default_value = "2")]
            #[ortho_config(cli_default_as_absent)]
            value: u32,
        }
    };

    let err = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow!("expected duplicate clap default error"))?;
    let err_text = err.to_string();
    ensure!(
        err_text.contains("duplicate clap default override"),
        "unexpected duplicate default error: {err_text}",
    );
    Ok(())
}

#[test]
fn clap_variant_name_cases() -> Result<()> {
    use proc_macro2::TokenStream;
    use quote::quote;

    let cases: &[(TokenStream, &str)] = &[
        (
            quote! { enum Commands { #[command(name = "take-leave")] TakeLeave(TakeLeaveArgs), } },
            "take-leave",
        ),
        (
            quote! { enum Commands { #[clap(name = "take-leave")] TakeLeave(TakeLeaveArgs), } },
            "take-leave",
        ),
    ];

    for (tokens, expected_name) in cases {
        let input: DeriveInput = syn::parse2(tokens.clone())?;
        let name = clap_variant_name(first_variant(&input)?)?
            .ok_or_else(|| anyhow!("missing variant name"))?;
        ensure!(
            name.value() == *expected_name,
            "expected name `{expected_name}`, got `{}`",
            name.value(),
        );
    }
    Ok(())
}

#[test]
fn clap_field_is_subcommand_cases() -> Result<()> {
    use proc_macro2::TokenStream;
    use quote::quote;

    let cases: &[(TokenStream, bool)] = &[
        (
            quote! { struct Cli { #[command(subcommand)] command: Commands, } },
            true,
        ),
        (
            quote! { struct Cli { #[clap(subcommand)] command: Commands, } },
            true,
        ),
        (
            quote! { struct Cli { #[command(subcommand, long = "cmd")] command: Commands, } },
            true,
        ),
        (quote! { struct Cli { #[arg(long)] name: String, } }, false),
    ];

    for (tokens, expected) in cases {
        let input: DeriveInput = syn::parse2(tokens.clone())?;
        let actual = clap_field_is_subcommand(first_field(&input)?)?;
        ensure!(
            actual == *expected,
            "input `{tokens}`: expected subcommand={expected}, got {actual}",
        );
    }
    Ok(())
}
