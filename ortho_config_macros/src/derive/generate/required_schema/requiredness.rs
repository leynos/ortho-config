//! Activation decision for one generated schema field.
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

/// The activation a declared field contributes to its schema.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FieldActivation {
    /// No layer supplies the value, so its absence is an error.
    Required,
    /// A layer supplies the value whichever loader reads the type.
    Optional,
    /// Only the declaring type's own loader supplies the value, by materializing
    /// the field's declared default.
    DefaultBacked,
}

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

/// Classify a field's activation.
///
/// The checks mirror the layers that can supply a value: an `Option` type, a
/// Serde default, or an explicit or inferred default.
///
/// Only `#[derive(OrthoConfig)]` emits the loader that materializes declared
/// defaults, so only that derive reports [`FieldActivation::DefaultBacked`]: a
/// field is optional for its own loader and required wherever another loader
/// reads the type. A type that derives the schema alone is only ever read by
/// its parent's loader, which never builds a defaults layer for it, so its
/// declared default is not a supply layer and the field stays required.
///
/// A default that `ortho_config` materializes is serialized under the Rust
/// identifier, so it only supplies the value when the loader reads that
/// identifier; a renamed field whose identifier is not an alias stays required
/// even though it declares a default.
///
/// An `Option` type is optional only because Serde's `missing_field` helper
/// yields `None` for it — and `expr_is_missing` consults a field's custom
/// deserializer *instead* of that helper, returning `missing_field` directly.
/// A default is checked before the deserializer, so it still makes the field
/// optional. Without one, `Option<T>` is required.
pub(super) fn activation(
    field: &Field,
    attrs: &FieldAttrs,
    facts: &ActivationFacts<'_>,
) -> syn::Result<FieldActivation> {
    if is_optional_for_every_loader(field, facts)? {
        return Ok(FieldActivation::Optional);
    }
    Ok(if is_own_loader_only(field, attrs, facts) {
        FieldActivation::DefaultBacked
    } else {
        FieldActivation::Required
    })
}

/// Report whether a layer supplies the value whichever loader reads the type.
///
/// Either the field's own type yields a value for absence, or Serde does, and
/// neither depends on which loader is running.
fn is_optional_for_every_loader(field: &Field, facts: &ActivationFacts<'_>) -> syn::Result<bool> {
    let supplied_by_type =
        option_inner(&field.ty).is_some() && !serde_field_has_custom_deserializer(&field.attrs)?;
    let supplied_by_serde = serde_has_default(&field.attrs)? || facts.container_default;
    Ok(supplied_by_type || supplied_by_serde)
}

/// Report whether only the declaring type's own loader supplies the value.
fn is_own_loader_only(field: &Field, attrs: &FieldAttrs, facts: &ActivationFacts<'_>) -> bool {
    facts.materializes_defaults
        && declares_default(attrs)
        && default_reaches_field(field, facts.key, facts.aliases)
}

/// Report whether the field declares a default this crate can materialize.
const fn declares_default(attrs: &FieldAttrs) -> bool {
    attrs.default.is_some() || attrs.inferred_clap_default.is_some()
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
