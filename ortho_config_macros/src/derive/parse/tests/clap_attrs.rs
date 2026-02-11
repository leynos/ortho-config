//! Tests for clap attribute parsing helpers.

use super::super::parse_input;
use anyhow::{Result, anyhow, ensure};
use quote::ToTokens;
use syn::{DeriveInput, parse_quote};

fn expr_tokens(expr: &syn::Expr) -> String {
    expr.to_token_stream().to_string()
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

    let (_, _, _, attrs_vec) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let attrs = attrs_vec
        .first()
        .ok_or_else(|| anyhow!("missing field attributes"))?;
    let inferred = attrs
        .default
        .as_ref()
        .ok_or_else(|| anyhow!("missing inferred default"))?;

    let expected: syn::Expr = parse_quote! {
        ::core::convert::Into::into(String::from("!"))
    };
    ensure!(
        expr_tokens(inferred) == expr_tokens(&expected),
        "expected inferred default {}, got {}",
        expr_tokens(&expected),
        expr_tokens(inferred),
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

    let (_, _, _, attrs_vec) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let attrs = attrs_vec
        .first()
        .ok_or_else(|| anyhow!("missing field attributes"))?;
    let inferred = attrs
        .default
        .as_ref()
        .ok_or_else(|| anyhow!("missing inferred default"))?;
    let inferred_tokens = expr_tokens(inferred);
    ensure!(
        inferred_tokens.contains("IntoIterator :: into_iter"),
        "expected inferred default_values_t expression to use IntoIterator, got {inferred_tokens}",
    );
    ensure!(
        inferred_tokens.contains("collect :: < :: std :: vec :: Vec < _ > >"),
        "expected inferred default_values_t expression to collect into Vec, got {inferred_tokens}",
    );
    Ok(())
}

#[test]
fn infers_default_from_clap_default_value_when_requested() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Demo {
            #[arg(default_value = "42")]
            #[ortho_config(cli_default_as_absent)]
            answer: u32,
        }
    };

    let (_, _, _, attrs_vec) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let attrs = attrs_vec
        .first()
        .ok_or_else(|| anyhow!("missing field attributes"))?;
    let inferred = attrs
        .default
        .as_ref()
        .ok_or_else(|| anyhow!("missing inferred default"))?;
    let inferred_tokens = expr_tokens(inferred);

    ensure!(
        inferred_tokens.contains("FromStr"),
        "expected inferred default_value expression to use FromStr, got {inferred_tokens}",
    );
    ensure!(
        inferred_tokens.contains("42"),
        "expected inferred default_value expression to retain the literal value, got {inferred_tokens}",
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

    let (_, _, _, attrs_vec) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let attrs = attrs_vec
        .first()
        .ok_or_else(|| anyhow!("missing field attributes"))?;
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

    let (_, _, _, attrs_vec) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let attrs = attrs_vec
        .first()
        .ok_or_else(|| anyhow!("missing field attributes"))?;
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
