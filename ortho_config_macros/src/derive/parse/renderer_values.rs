//! Leaf-value parsing for the `renderer(...)` attribute.
//!
//! Split from `renderer_attrs` to keep both modules under the repository's
//! 400-line ceiling. Everything here validates a single policy value; the
//! group and record structure lives in the parent module.

use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{LitBool, LitInt, LitStr, Token};

use super::doc_types::{RenderWidthAttr, RendererFieldAttr};
use super::literals::lit_str;

/// Largest fixed width accepted by the derive input.
///
/// Matches the IR's `NonZeroU16` column count.
const MAX_COLUMNS: u64 = u16::MAX as u64;

/// Accepted `tty` tokens.
pub(super) const TTY_VALUES: &[&str] = &["independent", "adapts", "requires"];
/// Accepted `closed_stdin` tokens.
pub(super) const CLOSED_STDIN_VALUES: &[&str] = &["ignored", "eof", "error"];
/// Accepted colour, emoji, progress, and pager tokens.
pub(super) const TOGGLE_VALUES: &[&str] = &["never", "auto", "always"];
/// Accepted `accessibility` tokens.
pub(super) const ACCESSIBILITY_VALUES: &[&str] = &["unspecified", "text_equivalent"];
/// Accepted `plain` tokens.
pub(super) const PLAIN_VALUES: &[&str] = &["unsupported", "available", "always"];
/// Accepted `stdout` and `stderr` tokens.
pub(super) const STREAM_VALUES: &[&str] = &["none", "text", "structured", "mixed"];
/// Accepted inheritance tokens.
pub(super) const INHERITANCE_VALUES: &[&str] = &["allowed", "forbidden"];
/// Accepted `localization` tokens for a declared output field.
pub(super) const LOCALIZATION_VALUES: &[&str] = &["localized", "non_localized"];

/// Reads one enum token and rejects any spelling outside `accepted`.
pub(super) fn parse_token(
    nested: &ParseNestedMeta,
    key: &str,
    accepted: &[&str],
    attribute: &str,
) -> syn::Result<String> {
    let value_lit = lit_str(nested, key)?;
    let value = value_lit.value();
    if accepted.contains(&value.as_str()) {
        return Ok(value);
    }
    Err(syn::Error::new(
        value_lit.span(),
        format!(
            "unknown {attribute} '{value}'; expected one of {}",
            quote_values(accepted)
        ),
    ))
}

/// Renders accepted tokens as a quoted, comma-separated list for diagnostics.
fn quote_values(accepted: &[&str]) -> String {
    accepted
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Parses a bare boolean attribute as `true`, or its explicit literal value.
pub(super) fn parse_bool(nested: &ParseNestedMeta) -> syn::Result<bool> {
    if nested.input.peek(Token![=]) {
        return Ok(nested.value()?.parse::<LitBool>()?.value());
    }
    Ok(true)
}

/// Parses a declared width, accepting `"terminal"`, `"unbounded"`, or a count.
///
/// Zero and values above `u16::MAX` are rejected at their own spans, so a width
/// that the IR's `NonZeroU16` cannot hold never reaches code generation.
pub(super) fn parse_width(nested: &ParseNestedMeta) -> syn::Result<RenderWidthAttr> {
    let value = nested.value()?;
    if value.peek(LitStr) {
        let text: LitStr = value.parse()?;
        return match text.value().as_str() {
            "terminal" => Ok(RenderWidthAttr::Terminal),
            "unbounded" => Ok(RenderWidthAttr::Unbounded),
            other => Err(syn::Error::new(
                text.span(),
                format!(
                    "unknown width '{other}'; expected \"terminal\", \"unbounded\", or a \
                     positive integer column count"
                ),
            )),
        };
    }
    let count: LitInt = value.parse()?;
    let parsed = count.base10_parse::<u64>()?;
    if parsed == 0 {
        return Err(syn::Error::new(
            count.span(),
            "width columns must be greater than zero",
        ));
    }
    if parsed > MAX_COLUMNS {
        return Err(syn::Error::new(
            count.span(),
            format!("width columns must not exceed {MAX_COLUMNS}"),
        ));
    }
    let columns = u16::try_from(parsed)
        .map_err(|_| syn::Error::new(count.span(), "width columns must fit in a 16-bit integer"))?;
    Ok(RenderWidthAttr::Fixed(columns))
}

/// Parses one `field(<id>, localization = "<token>")` entry.
pub(super) fn parse_field_entry(nested: &ParseNestedMeta) -> syn::Result<RendererFieldAttr> {
    let content;
    syn::parenthesized!(content in nested.input);
    let id_lit: LitStr = content.parse()?;
    let id = id_lit.value();
    if id.is_empty() {
        return Err(syn::Error::new(
            id_lit.span(),
            "field id must not be empty; it is a stable output field identifier",
        ));
    }

    let mut localization = None;
    while !content.is_empty() {
        content.parse::<Token![,]>()?;
        if content.is_empty() {
            break;
        }
        let key: syn::Ident = content.parse()?;
        if key != "localization" {
            return Err(syn::Error::new(
                key.span(),
                format!("unknown field policy `{key}`; expected `localization`"),
            ));
        }
        content.parse::<Token![=]>()?;
        let value_lit: LitStr = content.parse()?;
        let value = value_lit.value();
        if !LOCALIZATION_VALUES.contains(&value.as_str()) {
            return Err(syn::Error::new(
                value_lit.span(),
                format!(
                    "unknown localization '{value}'; expected one of {}",
                    quote_values(LOCALIZATION_VALUES)
                ),
            ));
        }
        if localization.is_some() {
            return Err(syn::Error::new(
                key.span(),
                "duplicate field policy `localization`; assign it once per field",
            ));
        }
        localization = Some(value);
    }

    let declared_localization = localization.ok_or_else(|| {
        syn::Error::new(
            id_lit.span(),
            "field requires localization = \"localized\" or \"non_localized\"",
        )
    })?;
    Ok(RendererFieldAttr {
        id,
        localization: declared_localization,
    })
}

/// Rejects a repeated output field identifier within one renderer record.
///
/// Comparison is exact: identifiers are opaque and case-sensitive, so the
/// parser never normalizes them.
pub(super) fn reject_duplicate_fields(
    fields: &[RendererFieldAttr],
    nested: &ParseNestedMeta,
) -> syn::Result<()> {
    let Some(latest) = fields.last() else {
        return Ok(());
    };
    let repeats = fields
        .iter()
        .rev()
        .skip(1)
        .any(|earlier| earlier.id == latest.id);
    if repeats {
        return Err(syn::Error::new(
            nested.path.span(),
            format!(
                "duplicate output field '{}'; field identifiers must be unique within one \
                 renderer",
                latest.id
            ),
        ));
    }
    Ok(())
}
