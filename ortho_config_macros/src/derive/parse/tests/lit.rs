//! Tests for literal parsing helpers.

use super::super::*;
use anyhow::{Result, anyhow, ensure};
use syn::Attribute;

#[test]
fn lit_str_parses_string_values() -> Result<()> {
    let attr: Attribute = syn::parse_quote!(#[ortho_config(cli_long = "name")]);
    let mut observed = None;
    attr.parse_nested_meta(|meta| {
        let s = __doc_lit_str(&meta, "cli_long")?;
        observed = Some(s.value());
        Ok(())
    })
    .map_err(|err| anyhow!("expected attribute parsing to succeed: {err}"))?;
    let value = observed.ok_or_else(|| anyhow!("cli_long attribute callback was not invoked"))?;
    ensure!(value == "name", "unexpected cli_long value: {value}");
    Ok(())
}

#[test]
fn lit_char_parses_char_values() -> Result<()> {
    let attr: syn::Attribute = syn::parse_quote!(#[ortho_config(cli_short = 'n')]);
    let mut observed = None;
    attr.parse_nested_meta(|meta| {
        let c = lit_char(&meta, "cli_short")?;
        observed = Some(c);
        Ok(())
    })
    .map_err(|err| anyhow!("expected attribute parsing to succeed: {err}"))?;
    let value = observed.ok_or_else(|| anyhow!("cli_short attribute callback was not invoked"))?;
    ensure!(value == 'n', "unexpected cli_short value: {value}");
    Ok(())
}
