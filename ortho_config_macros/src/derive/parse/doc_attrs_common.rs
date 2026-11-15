//! Parsers for documentation elements shared by struct and field attributes.
//!
//! Examples, links, and notes may be declared on a struct or on a field, so
//! their parsers live here rather than being duplicated in, or owned by, either
//! attribute family.

use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;

use super::discard_unknown;
use super::doc_types::{DocExampleAttr, DocLinkAttr, DocNoteAttr};
use super::literals::lit_str;

/// Parses one example and requires its code while leaving localized labels
/// optional.
pub(super) fn parse_example_meta(meta: &ParseNestedMeta) -> syn::Result<DocExampleAttr> {
    let mut title_id = None;
    let mut code = None;
    let mut body_id = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("title_id") {
            title_id = Some(lit_str(&nested, "title_id")?.value());
            return Ok(());
        }
        if nested.path.is_ident("code") {
            code = Some(lit_str(&nested, "code")?.value());
            return Ok(());
        }
        if nested.path.is_ident("body_id") {
            body_id = Some(lit_str(&nested, "body_id")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })?;
    let code_value =
        code.ok_or_else(|| syn::Error::new(meta.path.span(), "example requires code"))?;
    Ok(DocExampleAttr {
        title_id,
        code: code_value,
        body_id,
    })
}

/// Parses a link with a required URI and an optional localized link label.
pub(super) fn parse_link_meta(meta: &ParseNestedMeta) -> syn::Result<DocLinkAttr> {
    let mut text_id = None;
    let mut uri = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("text_id") {
            text_id = Some(lit_str(&nested, "text_id")?.value());
            return Ok(());
        }
        if nested.path.is_ident("uri") {
            uri = Some(lit_str(&nested, "uri")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })?;
    let uri_value = uri.ok_or_else(|| syn::Error::new(meta.path.span(), "link requires uri"))?;
    Ok(DocLinkAttr {
        text_id,
        uri: uri_value,
    })
}

/// Parses a localized note reference, rejecting notes without a message ID.
pub(super) fn parse_note_meta(meta: &ParseNestedMeta) -> syn::Result<DocNoteAttr> {
    let mut text_id = None;
    meta.parse_nested_meta(|nested| {
        if nested.path.is_ident("text_id") {
            text_id = Some(lit_str(&nested, "text_id")?.value());
            return Ok(());
        }
        discard_unknown(&nested)
    })?;
    let text_id_value =
        text_id.ok_or_else(|| syn::Error::new(meta.path.span(), "note requires text_id"))?;
    Ok(DocNoteAttr {
        text_id: text_id_value,
    })
}
