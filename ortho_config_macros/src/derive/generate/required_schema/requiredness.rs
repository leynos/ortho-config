//! Requiredness decision for one generated schema field.
//!
//! A field's absence is an error unless some layer can supply it. The layers
//! are the field's own type, a Serde default, and — only when the companion
//! derive emits the defaults struct — a default this crate materializes.
//! Keeping the decision here separates which keys can supply a value from the
//! token emission that records it.

use syn::Field;

use crate::derive::parse::{
    FieldAttrs, option_inner, serde_field_has_custom_deserializer, serde_has_default,
};

/// The declared facts that decide whether a field's absence is an error.
///
/// The key and aliases travel together with the container default because
/// they answer one question: which merged keys can supply this field.
pub(super) struct ActivationFacts<'a> {
    /// Whether the container carries `#[serde(default)]`.
    pub(super) container_default: bool,
    /// Whether the companion derive emits the loader that materializes
    /// declared defaults.
    pub(super) materializes_defaults: bool,
    /// The key the loader reads for this field.
    pub(super) key: &'a str,
    /// Alternative spellings the loader accepts for the same value.
    pub(super) aliases: &'a [String],
}

/// Classify a field as optional or required.
///
/// The checks mirror the layers that can supply a value: an `Option` type, an
/// explicit or inferred default, or a Serde default at either level.
///
/// A default that `ortho_config` materializes is serialized under the Rust
/// identifier, so it only supplies the value when the loader reads that
/// identifier; a renamed field whose identifier is not an alias stays required
/// even though it declares a default.
///
/// Only `#[derive(OrthoConfig)]` emits the loader that materializes declared
/// defaults. A type that derives the schema alone is read by its parent's
/// loader, which never builds a defaults layer for it, so its declared default
/// is not a supply layer and its field stays required unless a Serde default
/// applies.
///
/// An `Option` type is optional only because Serde's `missing_field` helper
/// yields `None` for it — and `expr_is_missing` consults a field's custom
/// deserializer *instead* of that helper, returning `missing_field` directly.
/// A default is checked before the deserializer, so it still makes the field
/// optional. Without one, `Option<T>` is required.
pub(super) fn is_optional(
    field: &Field,
    attrs: &FieldAttrs,
    facts: &ActivationFacts<'_>,
) -> syn::Result<bool> {
    let is_optional_by_serde = serde_has_default(&field.attrs)? || facts.container_default;
    let default_reaches = default_reaches_field(field, facts.key, facts.aliases);
    let declares_default = attrs.default.is_some() || attrs.inferred_clap_default.is_some();
    let is_optional_by_default = facts.materializes_defaults && declares_default && default_reaches;
    let is_optional_by_type =
        option_inner(&field.ty).is_some() && !serde_field_has_custom_deserializer(&field.attrs)?;
    Ok(is_optional_by_type || is_optional_by_serde || is_optional_by_default)
}

/// Report whether a materialized default reaches the key the loader reads.
///
/// The generated defaults struct serializes every field under its Rust
/// identifier, while Serde reads the field under its renamed key. A default
/// therefore supplies the value only when the identifier is the key itself or
/// one of the field's accepted aliases.
fn default_reaches_field(field: &Field, key: &str, aliases: &[String]) -> bool {
    let Some(ident) = field.ident.as_ref() else {
        return false;
    };
    let identifier = ident.to_string();
    identifier == key || aliases.contains(&identifier)
}
