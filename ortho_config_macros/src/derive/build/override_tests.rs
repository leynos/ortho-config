use super::r#override::{
    build_collection_logic, build_override_struct, collect_collection_strategies,
};
use crate::derive::parse::{FieldAttrs, StructAttrs, parse_input};
use anyhow::{Result, anyhow, ensure};
use quote::ToTokens;
use rstest::{fixture, rstest};

/// Convenience type used across tests to avoid recomputing the derive input.
type DemoInput = (Vec<syn::Field>, Vec<FieldAttrs>, StructAttrs);

#[fixture]
fn demo_input() -> DemoInput {
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
    let (_, fields, struct_attrs, field_attrs) =
        parse_input(&input).expect("fixture should parse derive input");
    (fields, field_attrs, struct_attrs)
}

#[rstest]
fn collect_collection_strategies_selects_collections(demo_input: DemoInput) -> Result<()> {
    let (fields, field_attrs, _) = demo_input;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    ensure!(strategies.append.len() == 1, "expected single append field");
    ensure!(
        strategies.map_replace.len() == 1,
        "expected single map replace field",
    );
    let (map_ident, map_ty) = strategies
        .map_replace
        .first()
        .expect("map replace strategies should contain an entry");
    ensure!(
        map_ident == "field3",
        "expected map field identifier to match",
    );
    ensure!(
        map_ty.to_token_stream().to_string() == "std :: collections :: BTreeMap < String , u32 >",
        "unexpected map field type: {map_ty:?}",
    );
    Ok(())
}

#[rstest]
fn build_collection_logic_includes_map_assignment(demo_input: DemoInput) -> Result<()> {
    let (fields, field_attrs, _) = demo_input;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    let tokens = build_collection_logic(&strategies);
    ensure!(
        tokens.post_extract.to_string().contains("cfg . field3"),
        "expected generated map reassignment",
    );
    Ok(())
}

#[rstest]
fn build_override_struct_creates_struct(demo_input: DemoInput) -> Result<()> {
    let (fields, field_attrs, _) = demo_input;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    let (ts, init_ts) = build_override_struct(&syn::parse_quote!(Demo), &strategies);
    ensure!(
        ts.to_string().contains("struct __DemoCollectionOverride"),
        "override struct missing expected identifier",
    );
    ensure!(
        init_ts.to_string().contains("__DemoCollectionOverride"),
        "override init missing expected struct",
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
    let (_, fields, _, field_attrs) = parse_input(&input)?;
    let Err(err) = collect_collection_strategies(&fields, &field_attrs) else {
        return Err(anyhow!("expected strategy validation to fail"));
    };
    ensure!(
        err.to_string()
            .contains("merge_strategy is only supported on Vec<_> or BTreeMap<_, _> fields"),
        "unexpected error: {err}",
    );
    Ok(())
}

#[test]
fn collect_collection_strategies_rejects_keyed_on_vec() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoKeyedVecError {
            #[ortho_config(merge_strategy = "keyed")]
            items: Vec<String>,
        }
    };
    let (_, fields, _, field_attrs) = parse_input(&input)?;
    let result = collect_collection_strategies(&fields, &field_attrs);
    let Err(err) = result else {
        return Err(anyhow!("expected keyed strategy on Vec to be rejected"));
    };
    ensure!(
        err.to_string()
            .contains("keyed merge strategy is not supported for Vec<_> fields"),
        "unexpected error message: {err}",
    );
    Ok(())
}

#[test]
fn collect_collection_strategies_rejects_append_on_btreemap() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoAppendMapError {
            #[ortho_config(merge_strategy = "append")]
            settings: std::collections::BTreeMap<String, i32>,
        }
    };
    let (_, fields, _, field_attrs) = parse_input(&input)?;
    let result = collect_collection_strategies(&fields, &field_attrs);
    let Err(err) = result else {
        return Err(anyhow!(
            "expected append strategy on BTreeMap to be rejected",
        ));
    };
    ensure!(
        err.to_string()
            .contains("append merge strategy is not supported for BTreeMap fields"),
        "unexpected error message: {err}",
    );
    Ok(())
}

#[test]
fn collect_collection_strategies_skips_replace_vec() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoReplaceVec {
            #[ortho_config(merge_strategy = "replace")]
            values: Vec<String>,
        }
    };
    let (_, fields, _, field_attrs) = parse_input(&input)?;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    ensure!(
        strategies.append.is_empty(),
        "vector replace strategy should not populate append list"
    );
    Ok(())
}

#[test]
fn collect_collection_strategies_skips_keyed_map_entry() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoKeyedMap {
            #[ortho_config(merge_strategy = "keyed")]
            values: std::collections::BTreeMap<String, i32>,
        }
    };
    let (_, fields, _, field_attrs) = parse_input(&input)?;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    ensure!(
        strategies.map_replace.is_empty(),
        "keyed map strategy should not populate replace list"
    );
    Ok(())
}
