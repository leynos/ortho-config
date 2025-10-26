//! Tests for collection override helpers.

use super::*;
use anyhow::{Result, anyhow, ensure};
use quote::ToTokens;
use rstest::rstest;

fn demo_input() -> Result<(Vec<syn::Field>, Vec<FieldAttrs>)> {
    let input: syn::DeriveInput = syn::parse_quote! {
        #[ortho_config(prefix = "CFG_")]
        struct Demo {
            #[ortho_config(cli_long = "opt", cli_short = 'o', default = 5)]
            field1: Option<u32>,
            #[ortho_config(merge_strategy = "append")]
            field2: Vec<String>,
            #[ortho_config(merge_strategy = "replace")]
            field3: std::collections::BTreeMap<String, u32>,
        }
    };
    let (_, fields, _, field_attrs) = crate::derive::parse::parse_input(&input)?;
    Ok((fields, field_attrs))
}

fn parse_single_field(
    input: &syn::DeriveInput,
    description: &str,
) -> Result<(syn::Field, FieldAttrs)> {
    let (_, fields, _, field_attrs) = crate::derive::parse::parse_input(input)?;
    let field = fields
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("missing {description} field"))?;
    let attrs = field_attrs
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("missing {description} attributes"))?;
    Ok((field, attrs))
}

#[test]
fn collect_collection_strategies_selects_collections() -> Result<()> {
    let (fields, field_attrs) = demo_input()?;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    ensure!(strategies.append.len() == 1, "expected single append field");
    ensure!(
        strategies.map_replace.len() == 1,
        "expected single map replace field"
    );
    let (map_ident, map_ty) = strategies
        .map_replace
        .first()
        .expect("map replace strategies should contain an entry");
    ensure!(
        map_ident == "field3",
        "expected map field identifier to match"
    );
    ensure!(
        map_ty.to_token_stream().to_string()
            == "std :: collections :: BTreeMap < String , u32 >",
        "unexpected map field type: {map_ty:?}"
    );
    Ok(())
}

#[test]
fn build_collection_logic_includes_map_assignment() -> Result<()> {
    let (fields, field_attrs) = demo_input()?;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    let tokens = build_collection_logic(&strategies);
    ensure!(
        tokens.post_extract.to_string().contains("cfg . field3"),
        "expected generated map reassignment"
    );
    Ok(())
}

#[test]
fn build_override_struct_creates_struct() -> Result<()> {
    let (fields, field_attrs) = demo_input()?;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    let (definition, initialiser) =
        build_override_struct(&syn::parse_quote!(Demo), &strategies);
    ensure!(
        definition
            .to_string()
            .contains("struct __DemoCollectionOverride"),
        "override struct missing expected identifier"
    );
    ensure!(
        initialiser
            .to_string()
            .contains("__DemoCollectionOverride"),
        "override init missing expected struct"
    );
    Ok(())
}

#[test]
fn collect_collection_strategies_errors_on_invalid_usage() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoAppendError {
            #[ortho_config(merge_strategy = "append")]
            field1: u32,
        }
    };
    let (_, fields, _, field_attrs) = crate::derive::parse::parse_input(&input)?;
    let Err(err) = collect_collection_strategies(&fields, &field_attrs) else {
        return Err(anyhow!("expected strategy validation to fail"));
    };
    ensure!(
        err.to_string()
            .contains("merge_strategy is only supported on Vec<_> or BTreeMap<_, _> fields"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[rstest]
#[case::identifies_append_vec(
    syn::parse_quote! {
        struct Demo {
            #[ortho_config(merge_strategy = "append")]
            field: Vec<String>,
        }
    },
    true,
    "field",
    syn::parse_quote!(String),
)]
#[case::skips_non_vec_without_strategy(
    syn::parse_quote! {
        struct DemoSkip {
            field: Option<String>,
        }
    },
    false,
    "",
    syn::parse_quote!(()),
)]
#[case::skips_replace_vec(
    syn::parse_quote! {
        struct DemoReplace {
            #[ortho_config(merge_strategy = "replace")]
            field: Vec<String>,
        }
    },
    false,
    "",
    syn::parse_quote!(()),
)]
fn process_vec_field_behaviour(
    #[case] input: syn::DeriveInput,
    #[case] expect_some: bool,
    #[case] expected_ident: &str,
    #[case] expected_ty: syn::Type,
) -> Result<()> {
    let (field, attrs) = parse_single_field(&input, "vector")?;
    let result = process_vec_field(&field, &attrs)?;

    if expect_some {
        let (ident, ty) = result.ok_or_else(|| anyhow!("expected Some"))?;
        ensure!(ident == expected_ident, "unexpected append target");
        ensure!(ty == expected_ty, "unexpected element type");
    } else {
        ensure!(result.is_none(), "expected vector field to be skipped");
    }
    Ok(())
}

#[test]
fn process_vec_field_errors_for_keyed_strategy() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoInvalid {
            #[ortho_config(merge_strategy = "keyed")]
            field: Vec<String>,
        }
    };
    let (field, attrs) = parse_single_field(&input, "invalid vector")?;
    let err = process_vec_field(&field, &attrs)
        .expect_err("keyed merge strategy is unsupported for Vec<_>");
    ensure!(
        err.to_string().contains("keyed merge strategy"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[test]
fn process_vec_field_errors_when_append_without_vec() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoAppendError {
            #[ortho_config(merge_strategy = "append")]
            field: u32,
        }
    };
    let (field, attrs) = parse_single_field(&input, "non-vector")?;
    let err = process_vec_field(&field, &attrs)
        .expect_err("append requires Vec field");
    ensure!(
        err.to_string()
            .contains("append merge strategy requires a Vec<_> field"),
        "unexpected error: {err}"
    );
    Ok(())
}

#[rstest]
#[case::replace_map(
    syn::parse_quote! {
        struct DemoReplace {
            #[ortho_config(merge_strategy = "replace")]
            field: std::collections::BTreeMap<String, u32>,
        }
    },
    true,
    "field",
)]
#[case::skip_keyed_map(
    syn::parse_quote! {
        struct DemoSkip {
            #[ortho_config(merge_strategy = "keyed")]
            field: std::collections::BTreeMap<String, u32>,
        }
    },
    false,
    "",
)]
fn process_map_field_behaviour(
    #[case] input: syn::DeriveInput,
    #[case] expect_some: bool,
    #[case] expected_ident: &str,
) -> Result<()> {
    let (field, attrs) = parse_single_field(&input, "map")?;
    let result = process_map_field(&field, &attrs)?;

    if expect_some {
        let (ident, _) = result.ok_or_else(|| anyhow!("expected Some"))?;
        ensure!(ident == expected_ident, "unexpected map override target");
    } else {
        ensure!(result.is_none(), "expected keyed map to be skipped");
    }
    Ok(())
}

#[test]
fn process_map_field_errors_for_append_strategy() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoInvalid {
            #[ortho_config(merge_strategy = "append")]
            field: std::collections::BTreeMap<String, u32>,
        }
    };
    let (field, attrs) = parse_single_field(&input, "invalid map")?;
    let err = process_map_field(&field, &attrs)
        .expect_err("append strategy is unsupported for maps");
    ensure!(
        err.to_string().contains("append merge strategy is not supported"),
        "unexpected error: {err}"
    );
    Ok(())
}
