use super::r#override::{
    CollectionStrategies, build_collection_logic, build_override_struct,
    collect_collection_strategies,
};
use crate::derive::parse::{FieldAttrs, StructAttrs, parse_input};
use anyhow::{Result, anyhow, ensure};
use quote::{ToTokens, quote};
use rstest::{fixture, rstest};

/// Convenience type used across tests to avoid recomputing the derive input.
type DemoInput = (Vec<syn::Field>, Vec<FieldAttrs>, StructAttrs);

/// Assert that collecting strategies for the provided input emits the expected
/// validation error.
#[expect(
    clippy::needless_pass_by_value,
    reason = "the helper signature must own the input to satisfy the review request"
)]
fn assert_collection_strategy_error(
    input: syn::DeriveInput,
    expected_error_substring: &str,
    context: &str,
) -> Result<()> {
    let (_, fields, _, field_attrs) = parse_input(&input)?;
    let result = collect_collection_strategies(&fields, &field_attrs);
    let Err(err) = result else {
        return Err(anyhow!("{context}"));
    };
    ensure!(
        err.to_string().contains(expected_error_substring),
        "unexpected error message: {err}",
    );
    Ok(())
}

/// Collect strategies for the provided input, propagating parse failures.
fn collect_strategies(input: syn::DeriveInput) -> Result<CollectionStrategies> {
    let (_, fields, _, field_attrs) = parse_input(&input)?;
    Ok(collect_collection_strategies(&fields, &field_attrs)?)
}

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
    ensure!(
        strategies.append.len() == 1,
        "expected single append field, found {}",
        strategies.append.len()
    );
    ensure!(
        strategies.map_replace.len() == 1,
        "expected single map replace field, found {}",
        strategies.map_replace.len()
    );
    let (map_ident, map_ty) = strategies
        .map_replace
        .first()
        .expect("map replace strategies should contain an entry");
    ensure!(
        map_ident == "field3",
        "expected map field identifier to match, found {map_ident}"
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
    let tokens = build_collection_logic(&strategies, &quote!(std::option::Option::<&()>::None));
    ensure!(
        tokens.post_extract.to_string().contains("cfg . field3"),
        "expected generated map reassignment"
    );
    Ok(())
}

#[rstest]
fn build_collection_logic_skips_empty_maps(demo_input: DemoInput) -> Result<()> {
    let (fields, field_attrs, _) = demo_input;
    let strategies = collect_collection_strategies(&fields, &field_attrs)?;
    let tokens = build_collection_logic(&strategies, &quote!(std::option::Option::<&()>::None));
    let pre_merge = tokens.pre_merge.to_string();
    let guard_count = pre_merge.matches("! v . is_empty ()").count();
    let assignment_count = pre_merge.matches("replace . field3 = Some (v").count();
    ensure!(
        guard_count == 0,
        "expected is_empty guard to be removed, found {guard_count}"
    );
    ensure!(
        assignment_count >= 3,
        "expected map assignments for each figment source, found {assignment_count}"
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
    assert_collection_strategy_error(
        input,
        "merge_strategy is only supported on Vec<_> or BTreeMap<_, _> fields",
        "expected strategy validation to fail",
    )
}

#[test]
fn collect_collection_strategies_rejects_keyed_on_vec() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoKeyedVecError {
            #[ortho_config(merge_strategy = "keyed")]
            items: Vec<String>,
        }
    };
    assert_collection_strategy_error(
        input,
        "keyed merge strategy is not supported for Vec<_> fields",
        "expected keyed strategy on Vec to be rejected",
    )
}

#[test]
fn collect_collection_strategies_rejects_append_on_btreemap() -> Result<()> {
    let input: syn::DeriveInput = syn::parse_quote! {
        struct DemoAppendMapError {
            #[ortho_config(merge_strategy = "append")]
            settings: std::collections::BTreeMap<String, i32>,
        }
    };
    assert_collection_strategy_error(
        input,
        "append merge strategy is not supported for BTreeMap fields",
        "expected append strategy on BTreeMap to be rejected",
    )
}

#[test]
fn collect_collection_strategies_skips_replace_vec() -> Result<()> {
    let strategies = collect_strategies(syn::parse_quote! {
        struct DemoReplaceVec {
            #[ortho_config(merge_strategy = "replace")]
            values: Vec<String>,
        }
    })?;
    ensure!(
        strategies.append.is_empty(),
        "vector replace strategy should not populate append list"
    );
    Ok(())
}

#[test]
fn collect_collection_strategies_skips_keyed_map_entry() -> Result<()> {
    let strategies = collect_strategies(syn::parse_quote! {
        struct DemoKeyedMap {
            #[ortho_config(merge_strategy = "keyed")]
            values: std::collections::BTreeMap<String, i32>,
        }
    })?;
    ensure!(
        strategies.map_replace.is_empty(),
        "keyed map strategy should not populate replace list"
    );
    Ok(())
}
