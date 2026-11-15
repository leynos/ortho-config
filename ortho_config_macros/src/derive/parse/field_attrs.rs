//! Field-level `#[ortho_config(...)]` attribute parsing.
//!
//! The struct-level keys live in `mod.rs`; the field-level keys live here so
//! the two vocabularies stay separately readable and the parent module stays
//! within its line budget. Both entry points share `parse_ortho_config` and
//! `discard_unknown` from the parent.

use syn::Token;

use super::literals::{lit_char, lit_str};
use super::localization::reject_localized_default;
use super::{
    FieldAttrs, MergeStrategy, apply_field_doc_attr, clap_default_value, clap_field_is_subcommand,
    discard_unknown, parse_ortho_config, reject_subcommand_ortho_config_attrs,
};

/// Parses field-level `#[ortho_config(...)]` attributes.
///
/// Recognised keys include `cli_long`, `cli_short`, `default`,
/// `merge_strategy`, `skip_cli`, and `cli_default_as_absent`. Unknown keys are
/// ignored, matching [`super::parse_struct_attrs`] for forwards compatibility.
/// This lenience may permit misspelt attribute names; users wanting stricter
/// validation can insert a manual `compile_error!` guard.
///
/// When `cli_default_as_absent` is active and no explicit `default` is
/// provided, this function attempts to infer a default from clap's default
/// attributes. String defaults retain the field parser metadata so generated
/// code can reproduce clap's conversion at runtime.
///
/// Used internally by the derive macro to extract configuration metadata
/// from field-level attributes.
pub(crate) fn parse_field_attrs(field: &syn::Field) -> Result<FieldAttrs, syn::Error> {
    let mut out = FieldAttrs {
        is_subcommand: clap_field_is_subcommand(field)?,
        ..FieldAttrs::default()
    };
    if out.is_subcommand {
        reject_subcommand_ortho_config_attrs(field)?;
        return Ok(out);
    }
    parse_ortho_config(&field.attrs, |meta| {
        if !apply_field_attr(meta, &mut out)? {
            // Unknown attributes are intentionally discarded to preserve
            // forwards compatibility while still allowing callers to add
            // new keys in future versions.
            discard_unknown(meta)?;
        }
        Ok(())
    })?;
    if out.cli_default_as_absent && out.default.is_none() {
        out.inferred_clap_default = clap_default_value(field)?;
    }
    Ok(out)
}

/// Applies a recognised field attribute, returning `true` if handled.
///
/// Unknown keys fall through to [`apply_field_doc_attr`], which reports
/// whether it recognised the key; a `false` there leaves the caller to
/// discard the value.
///
/// `localized_default` is recognised only to be rejected: it is deferred
/// (Decision D-4), and a key that is discarded as unknown would leave an
/// author who placed it on a field with no diagnostic at all — the value would
/// simply never appear in the catalogue. Rejecting here means the deferral is
/// reported wherever it is written, not only at struct level.
///
/// # Examples
///
/// ```rust,ignore
/// # use syn::meta::ParseNestedMeta;
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let mut out = FieldAttrs::default();
/// if !apply_field_attr(meta, &mut out)? {
///     // unknown attribute
/// }
/// # Ok(())
/// # }
/// ```
fn apply_field_attr(
    meta: &syn::meta::ParseNestedMeta,
    out: &mut FieldAttrs,
) -> Result<bool, syn::Error> {
    let Some(ident) = meta.path.get_ident() else {
        return Ok(false);
    };
    let key = ident.to_string();
    match key.as_str() {
        "localized_default" => Err(reject_localized_default(&meta.path)),
        "cli_long" => {
            let s = lit_str(meta, "cli_long")?;
            out.cli_long = Some(s.value());
            Ok(true)
        }
        "cli_short" => {
            let c = lit_char(meta, "cli_short")?;
            out.cli_short = Some(c);
            Ok(true)
        }
        "default" => {
            out.default = Some(meta.value()?.parse()?);
            Ok(true)
        }
        "merge_strategy" => {
            let s = lit_str(meta, "merge_strategy")?;
            out.merge_strategy = Some(MergeStrategy::parse(&s.value(), s.span())?);
            Ok(true)
        }
        "skip_cli" => {
            out.skip_cli = true;
            Ok(true)
        }
        "cli_default_as_absent" => {
            out.cli_default_as_absent = parse_cli_default_as_absent(meta)?;
            Ok(true)
        }
        _ => apply_field_doc_attr(meta, out),
    }
}

/// Parses the optional boolean value of `cli_default_as_absent`.
///
/// A bare `cli_default_as_absent` enables the behaviour; an explicit
/// `cli_default_as_absent = <bool>` sets it directly.
fn parse_cli_default_as_absent(meta: &syn::meta::ParseNestedMeta) -> Result<bool, syn::Error> {
    if meta.input.peek(Token![=]) {
        Ok(meta.value()?.parse::<syn::LitBool>()?.value)
    } else {
        Ok(true)
    }
}
