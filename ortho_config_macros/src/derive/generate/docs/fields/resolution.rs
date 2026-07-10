//! Resolution of field value types and requiredness for documentation IR.

use crate::derive::parse::{
    FieldAttrs, btree_map_inner, hash_map_inner, option_inner, serde_has_default, vec_inner,
};

use super::value_types::{ValueTypeModel, infer_value_type, parse_value_type_override};

pub(super) fn resolve_value_type(attrs: &FieldAttrs, field: &syn::Field) -> Option<ValueTypeModel> {
    attrs
        .doc
        .value_type
        .as_deref()
        .map(parse_value_type_override)
        .or_else(|| infer_value_type(&field.ty))
}

pub(super) fn resolve_required(field: &syn::Field, attrs: &FieldAttrs) -> syn::Result<bool> {
    if let Some(required) = attrs.doc.required {
        return Ok(required);
    }
    Ok(!infers_non_required(field, attrs)?)
}

/// Returns `true` when the field can be inferred as non-required (`Option<T>`,
/// `#[ortho_config(default = ...)]`, `#[serde(default)]`, or a collection type).
fn infers_non_required(field: &syn::Field, attrs: &FieldAttrs) -> syn::Result<bool> {
    if option_inner(&field.ty).is_some() {
        return Ok(true);
    }
    if attrs.default.is_some() {
        return Ok(true);
    }
    if attrs.inferred_clap_default.is_some() {
        return Ok(true);
    }
    if serde_has_default(&field.attrs)? {
        return Ok(true);
    }
    // Collections (Vec, BTreeMap, HashMap) default to non-required since they can be empty.
    if is_collection_type(&field.ty) {
        return Ok(true);
    }
    Ok(false)
}

/// Returns `true` if `ty` is a collection type (`Vec`, `BTreeMap`, `HashMap`).
fn is_collection_type(ty: &syn::Type) -> bool {
    vec_inner(ty).is_some() || btree_map_inner(ty).is_some() || hash_map_inner(ty).is_some()
}
