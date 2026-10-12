//! Parsing for struct-level localization attributes.
//!
//! Recognizes `localization_base` (the Fluent catalogue root for the derive)
//! and deliberately rejects `localized_default` with a deferral diagnostic
//! (Decision D-4) rather than silently discarding it.

use syn::meta::ParseNestedMeta;

use super::StructAttrs;
use super::literals::lit_str;

/// Builds the deferral diagnostic for `localized_default`.
///
/// The attribute is deliberately unimplemented (Decision D-4), and it is
/// rejected wherever it appears rather than silently discarded. Both the
/// struct-level parser and the field-level one call this, so the two rejection
/// sites cannot drift apart as the design section moves.
pub(super) fn reject_localized_default(path: &syn::Path) -> syn::Error {
    syn::Error::new_spanned(
        path,
        concat!(
            "`localized_default` is not yet implemented; see ",
            "cli-localization-design.md §8.2",
        ),
    )
}

/// Applies a struct-level localization attribute to `out`.
///
/// Returns `Ok(true)` when `meta` named a key owned by this module, and
/// `Ok(false)` otherwise so callers can fall through to other parsers.
pub(super) fn parse_localization_attr(
    meta: &ParseNestedMeta<'_>,
    out: &mut StructAttrs,
) -> syn::Result<bool> {
    let Some(key) = meta.path.get_ident().map(ToString::to_string) else {
        return Ok(false);
    };

    match key.as_str() {
        "localization_base" => {
            if out.localization_base.is_some() {
                return Err(syn::Error::new_spanned(
                    &meta.path,
                    "duplicate `localization_base` attribute",
                ));
            }
            let lit = lit_str(meta, "localization_base")?;
            out.localization_base = Some(lit.value());
            Ok(true)
        }
        "localized_default" => Err(reject_localized_default(&meta.path)),
        _ => Ok(false),
    }
}
