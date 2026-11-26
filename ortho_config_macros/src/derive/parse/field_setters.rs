//! Setters for the `#[ortho_config(...)]` field attributes.
//!
//! Each setter parses one recognised key and records it on [`FieldAttrs`],
//! so `apply_field_attr` stays a flat dispatch over attribute names.

use super::{FieldAttrs, MergeStrategy, lit_char, lit_str, parse_cli_default_as_absent};

/// Records the `cli_long` flag name.
pub(super) fn set_cli_long(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    out.cli_long = Some(lit_str(meta, "cli_long")?.value());
    Ok(())
}

/// Records the `cli_short` flag character.
pub(super) fn set_cli_short(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    out.cli_short = Some(lit_char(meta, "cli_short")?);
    Ok(())
}

/// Records the explicit `default` expression.
pub(super) fn set_default(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    out.default = Some(meta.value()?.parse()?);
    Ok(())
}

/// Records the `merge_strategy`, reporting an unknown name at its own span.
pub(super) fn set_merge_strategy(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    let name = lit_str(meta, "merge_strategy")?;
    out.merge_strategy = Some(MergeStrategy::parse(&name.value(), name.span())?);
    Ok(())
}

/// Records whether `cli_default_as_absent` is enabled.
pub(super) fn set_cli_default_as_absent(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<(), syn::Error> {
    out.cli_default_as_absent = parse_cli_default_as_absent(meta)?;
    Ok(())
}
