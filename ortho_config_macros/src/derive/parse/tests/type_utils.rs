//! Tests for type introspection helpers.

use super::super::*;
use anyhow::{Result, anyhow, ensure};
use rstest::rstest;
use syn::{Type, parse_quote};

#[rstest]
#[case(parse_quote!(Option<u32>))]
#[case(parse_quote!(std::option::Option<u32>))]
#[case(parse_quote!(core::option::Option<u32>))]
#[case(parse_quote!(crate::option::Option<u32>))]
fn option_inner_matches_various_prefixes(#[case] ty: Type) -> Result<()> {
    let expected: Type = parse_quote!(u32);
    let inner = option_inner(&ty).ok_or_else(|| anyhow!("expected Option"))?;
    ensure!(inner == &expected, "expected {expected:?}, got {inner:?}");
    Ok(())
}

#[rstest]
#[case(parse_quote!(Vec<u8>))]
#[case(parse_quote!(std::vec::Vec<u8>))]
#[case(parse_quote!(alloc::vec::Vec<u8>))]
#[case(parse_quote!(crate::vec::Vec<u8>))]
fn vec_inner_matches_various_prefixes(#[case] ty: Type) -> Result<()> {
    let expected: Type = parse_quote!(u8);
    let inner = vec_inner(&ty).ok_or_else(|| anyhow!("expected Vec"))?;
    ensure!(inner == &expected, "expected {expected:?}, got {inner:?}");
    Ok(())
}

#[rstest]
#[case::std(
    parse_quote!(std::collections::BTreeMap<String, u8>),
    parse_quote!(String),
    parse_quote!(u8),
)]
#[case::alloc(
    parse_quote!(alloc::collections::BTreeMap<u16, (u8, u8)>),
    parse_quote!(u16),
    parse_quote!((u8, u8)),
)]
#[case::crate_prefix(
    parse_quote!(crate::collections::BTreeMap<String, Vec<Option<u8>>>),
    parse_quote!(String),
    parse_quote!(Vec<Option<u8>>),
)]
fn btree_map_inner_matches_various_prefixes(
    #[case] ty: Type,
    #[case] expected_key: Type,
    #[case] expected_value: Type,
) -> Result<()> {
    let (key, value) = btree_map_inner(&ty).ok_or_else(|| anyhow!("expected BTreeMap"))?;
    ensure!(
        key == &expected_key,
        "key mismatch: {key:?} vs {expected_key:?}"
    );
    ensure!(
        value == &expected_value,
        "value mismatch: {value:?} vs {expected_value:?}",
    );
    Ok(())
}
