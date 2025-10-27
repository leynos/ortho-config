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
    Some(("field", syn::parse_quote!(String), true)),
)]
#[case::identifies_replace_vec(
    syn::parse_quote! {
        struct DemoReplace {
            #[ortho_config(merge_strategy = "replace")]
            field: Vec<String>,
        }
    },
    Some(("field", syn::parse_quote!(String), false)),
)]
#[case::skips_non_vec_without_strategy(
    syn::parse_quote! {
        struct DemoSkip {
            field: Option<String>,
        }
    },
    None,
)]
fn process_vec_field_behaviour(
    #[case] input: syn::DeriveInput,
    #[case] expected: Option<(&str, syn::Type, bool)>,
) -> Result<()> {
    let (field, attrs) = parse_single_field(&input, "vector")?;
    let name = field.ident.clone().ok_or_else(|| anyhow!("missing vector ident"))?;
    if let Some(vec_ty) = vec_inner(&field.ty) {
        let result = process_vec_field(&field, name, vec_ty, &attrs)?;
        if let Some((expected_ident, expected_ty, expected_append)) = expected {
            let (ident, ty, is_append) = result.ok_or_else(|| anyhow!("expected Some"))?;
            ensure!(ident == expected_ident, "unexpected vector target");
            ensure!(ty == expected_ty, "unexpected element type");
            ensure!(is_append == expected_append, "unexpected append flag");
        } else {
            ensure!(result.is_none(), "expected vector strategy to be skipped");
        }
    } else {
        ensure!(expected.is_none(), "expected non-vector field to be skipped");
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
    let name = field.ident.clone().ok_or_else(|| anyhow!("missing vector ident"))?;
    let vec_ty = vec_inner(&field.ty).expect("expected Vec field");
    let err = process_vec_field(&field, name, vec_ty, &attrs)
        .expect_err("keyed merge strategy is unsupported for Vec<_>");
    ensure!(
        err.to_string().contains("keyed merge strategy"),
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
    Some("field"),
)]
#[case::skip_keyed_map(
    syn::parse_quote! {
        struct DemoSkip {
            #[ortho_config(merge_strategy = "keyed")]
            field: std::collections::BTreeMap<String, u32>,
        }
    },
    None,
)]
fn process_map_field_behaviour(
    #[case] input: syn::DeriveInput,
    #[case] expected_ident: Option<&str>,
) -> Result<()> {
    let (field, attrs) = parse_single_field(&input, "map")?;
    let name = field.ident.clone().ok_or_else(|| anyhow!("missing map ident"))?;
    let result = process_btree_map_field(&field, name, &field.ty, &attrs)?;

    if let Some(expected_ident) = expected_ident {
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
    let name = field.ident.clone().ok_or_else(|| anyhow!("missing map ident"))?;
    let err = process_btree_map_field(&field, name, &field.ty, &attrs)
        .expect_err("append strategy is unsupported for maps");
    ensure!(
        err.to_string().contains("append merge strategy is not supported"),
        "unexpected error: {err}"
    );
    Ok(())
}
