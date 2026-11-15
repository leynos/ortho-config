//! Serde attribute parsing helpers.
//!
//! The derive macro uses these utilities to compute the JSON keys serde will
//! emit when serializing the parsed CLI struct. This is required for
//! `cli_default_as_absent` extraction: we serialize `self` to JSON and then
//! pluck individual fields out of the resulting object, so the lookup key must
//! respect `#[serde(rename = "...")]` and `#[serde(rename_all = "...")]`.

use syn::ext::IdentExt;
use syn::meta::ParseNestedMeta;
use syn::{Attribute, Field, LitStr, Token};

/// Supported `#[serde(rename_all = "...")]` rules for struct fields.
///
/// This is used by the `cli_default_as_absent` extraction implementation to
/// compute the same JSON field names that serde uses during serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SerdeRenameAll {
    /// Leaves a struct field untouched; Serde lowercases only enum variants.
    Lower,
    /// Uppercases every character in the field name.
    Upper,
    /// Capitalizes the first letter of each underscore-separated word.
    Pascal,
    /// Applies [`Self::Pascal`], then lowercases the first character.
    Camel,
    /// Leaves a struct field untouched; Serde reshapes only enum variants.
    Snake,
    /// Uppercases every character in the field name.
    ScreamingSnake,
    /// Replaces underscores with hyphens.
    Kebab,
    /// Uppercases every character, then replaces underscores with hyphens.
    ScreamingKebab,
}

impl SerdeRenameAll {
    /// Parses one of the supported Serde struct-field naming conventions.
    fn parse(value: &LitStr) -> syn::Result<Self> {
        match value.value().as_str() {
            "lowercase" => Ok(Self::Lower),
            "UPPERCASE" => Ok(Self::Upper),
            "PascalCase" => Ok(Self::Pascal),
            "camelCase" => Ok(Self::Camel),
            "snake_case" => Ok(Self::Snake),
            "SCREAMING_SNAKE_CASE" => Ok(Self::ScreamingSnake),
            "kebab-case" => Ok(Self::Kebab),
            "SCREAMING-KEBAB-CASE" => Ok(Self::ScreamingKebab),
            other => Err(syn::Error::new(
                value.span(),
                format!(
                    "unsupported serde rename_all value '{other}'; expected one of \
\"lowercase\", \"UPPERCASE\", \"PascalCase\", \"camelCase\", \"snake_case\", \
\"SCREAMING_SNAKE_CASE\", \"kebab-case\", or \"SCREAMING-KEBAB-CASE\""
                ),
            )),
        }
    }

    /// Applies Serde's struct-field rule to a Rust field identifier.
    ///
    /// These are Serde's own field rules, not general case conversions. Two of
    /// them leave the field alone, and `PascalCase` only capitalizes the first
    /// letter of each underscore-separated word without lowering the rest. A
    /// general case converter disagrees on mixed-case identifiers such as
    /// `api_URL`, which would make the schema name a key Serde never reads.
    fn apply(self, field_name: &str) -> String {
        match self {
            Self::Lower | Self::Snake => field_name.to_owned(),
            Self::Upper | Self::ScreamingSnake => field_name.to_ascii_uppercase(),
            Self::Pascal => pascal_case(field_name),
            Self::Camel => lower_first(&pascal_case(field_name)),
            Self::Kebab => field_name.replace('_', "-"),
            Self::ScreamingKebab => field_name.to_ascii_uppercase().replace('_', "-"),
        }
    }
}

/// Capitalize the first letter of each underscore-separated word.
///
/// Later characters keep their case, so `api_URL` becomes `ApiURL` rather than
/// the `ApiUrl` a general-purpose converter would produce.
fn pascal_case(field_name: &str) -> String {
    let mut pascal = String::new();
    let mut is_word_start = true;
    for character in field_name.chars() {
        if character == '_' {
            is_word_start = true;
        } else if is_word_start {
            pascal.push(character.to_ascii_uppercase());
            is_word_start = false;
        } else {
            pascal.push(character);
        }
    }
    pascal
}

/// Lowercase the first character and keep the rest as it is.
fn lower_first(value: &str) -> String {
    let mut characters = value.chars();
    characters.next().map_or_else(String::new, |first| {
        first.to_ascii_lowercase().to_string() + characters.as_str()
    })
}

/// One Serde attribute value separated by serialization direction.
///
/// A bare `key = "..."` sets both directions. The directional form, written
/// `key(serialize = "...")` or `key(deserialize = "...")`, sets one side and
/// leaves the other at Serde's default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Directional<T> {
    /// Value applied when serializing.
    pub serialize: Option<T>,
    /// Value applied when deserializing.
    pub deserialize: Option<T>,
}

/// Container-level `rename_all` rules, one per direction.
pub(crate) type SerdeRenameRules = Directional<SerdeRenameAll>;

/// Parses both directions of `#[serde(rename_all = "...")]`.
pub(crate) fn serde_rename_rules(attrs: &[Attribute]) -> syn::Result<SerdeRenameRules> {
    parse_directional(attrs, "rename_all", SerdeRenameAll::parse)
}

/// Parses one directional Serde key into its two direction slots.
///
/// `parse_value` decodes the literal for the attribute in question, so the
/// shared traversal serves both `rename_all` and `rename` without repeating
/// the bare and directional forms.
fn parse_directional<T: Clone>(
    attrs: &[Attribute],
    key: &str,
    parse_value: impl Fn(&LitStr) -> syn::Result<T>,
) -> syn::Result<Directional<T>> {
    // Spelled out rather than `default()`: the derived `Default` bound would
    // demand `T: Default`, which a plain enum value need not implement.
    let mut rules = Directional {
        serialize: None,
        deserialize: None,
    };
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident(key) {
                super::discard_unknown(&meta)?;
                return Ok(());
            }
            if meta.input.peek(Token![=]) {
                let value = parse_value(&meta.value()?.parse::<LitStr>()?)?;
                rules.serialize = Some(value.clone());
                rules.deserialize = Some(value);
                return Ok(());
            }
            if !meta.input.peek(syn::token::Paren) {
                return Ok(());
            }
            meta.parse_nested_meta(|nested| set_direction(&nested, &mut rules, &parse_value))
        })?;
    }
    Ok(rules)
}

/// Records one direction of the `serialize = "..."` / `deserialize = "..."`
/// form, leaving unknown directions discarded like any other option.
fn set_direction<T>(
    nested: &ParseNestedMeta,
    rules: &mut Directional<T>,
    parse_value: &impl Fn(&LitStr) -> syn::Result<T>,
) -> syn::Result<()> {
    let slot = if nested.path.is_ident("serialize") {
        &mut rules.serialize
    } else if nested.path.is_ident("deserialize") {
        &mut rules.deserialize
    } else {
        return super::discard_unknown(nested);
    };
    *slot = Some(parse_value(&nested.value()?.parse::<LitStr>()?)?);
    Ok(())
}

/// Parses the serialization-side `#[serde(rename_all = "...")]` rule.
///
/// Generated CLI extraction looks up keys in the serialized configuration
/// object, so this is the rule it must reproduce.
pub(crate) fn serde_rename_all(attrs: &[Attribute]) -> syn::Result<Option<SerdeRenameAll>> {
    Ok(serde_rename_rules(attrs)?.serialize)
}

/// Field-level `rename` names, one per direction.
pub(crate) type SerdeFieldRename = Directional<String>;

/// Parses `#[serde(rename = "...")]` and its directional forms.
///
/// A bare `rename = "..."` sets both directions. `rename(serialize = "...")`
/// leaves the deserialization name at the container default, and vice versa.
pub(crate) fn serde_field_rename_rules(attrs: &[Attribute]) -> syn::Result<SerdeFieldRename> {
    parse_directional(attrs, "rename", |value| Ok(value.value()))
}

/// Parses the serialization-side field rename.
///
/// Generated CLI extraction looks up keys in the serialized configuration
/// object, so a deserialize-only name does not change the serialized key.
pub(crate) fn serde_field_rename(attrs: &[Attribute]) -> syn::Result<Option<String>> {
    Ok(serde_field_rename_rules(attrs)?.serialize)
}

/// Parses `#[serde(alias = "...")]` values in declaration order.
pub(crate) fn serde_field_aliases(attrs: &[Attribute]) -> syn::Result<Vec<String>> {
    let mut aliases = Vec::new();
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("alias") {
                let value = meta.value()?.parse::<LitStr>()?;
                aliases.push(value.value());
            } else {
                super::discard_unknown(&meta)?;
            }
            Ok(())
        })?;
    }
    Ok(aliases)
}

/// Returns true if the field is excluded from deserialization.
///
/// Both `skip` and `skip_deserializing` mean the loader never reads a value
/// for the field, so no missing-value report can be truthful for it.
pub(crate) fn serde_field_skips_deserialization(attrs: &[Attribute]) -> syn::Result<bool> {
    serde_field_has_bare_flag(attrs, &["skip", "skip_deserializing"])
}

/// Returns true if the field composes a flattened Serde object.
///
/// A flattened field's own name never appears as a key, so requirement
/// enumeration must not report it unless the field also carries the nested
/// marker.
pub(crate) fn serde_field_is_flattened(attrs: &[Attribute]) -> syn::Result<bool> {
    serde_field_has_bare_flag(attrs, &["flatten"])
}

/// Detects whether any of `flags` appears as a bare Serde field option.
fn serde_field_has_bare_flag(attrs: &[Attribute], flags: &[&str]) -> syn::Result<bool> {
    let mut found = false;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            let is_flag = meta
                .path
                .get_ident()
                .is_some_and(|ident| flags.iter().any(|flag| ident == flag));
            if is_flag {
                found = true;
            } else {
                super::discard_unknown(&meta)?;
            }
            Ok(())
        })?;
    }
    Ok(found)
}

/// Computes the JSON key Serde accepts for `field` when deserializing.
///
/// An explicit deserialization name wins over the container rule; the rule is
/// applied to the unrawed identifier, which is what Serde itself does.
pub(crate) fn serde_deserialized_field_key(
    field: &Field,
    rename_all: Option<SerdeRenameAll>,
) -> syn::Result<String> {
    let Some(ident) = field.ident.as_ref() else {
        return Err(syn::Error::new_spanned(
            field,
            "unnamed fields are not supported",
        ));
    };
    if let Some(rename) = serde_field_rename_rules(&field.attrs)?.deserialize {
        return Ok(rename);
    }
    let field_name = ident.unraw().to_string();
    Ok(rename_all
        .map(|rule| rule.apply(&field_name))
        .unwrap_or(field_name))
}

/// Computes the JSON key Serde uses for `field` given an optional
/// container-level rename rule.
///
/// An explicit serialization name wins; otherwise the container rule is
/// applied, with the Rust identifier as the final fallback.
pub(crate) fn serde_serialized_field_key(
    field: &Field,
    rename_all: Option<SerdeRenameAll>,
) -> syn::Result<String> {
    let Some(ident) = field.ident.as_ref() else {
        return Err(syn::Error::new_spanned(
            field,
            "unnamed fields are not supported",
        ));
    };
    if let Some(rename) = serde_field_rename(&field.attrs)? {
        return Ok(rename);
    }
    // `serde_derive` unraws the identifier before applying `rename_all` and
    // before using it as the serialized key, so `r#type` serializes under
    // `type`. Keeping the raw spelling here would both break the rename rule
    // (`heck` splits words on the `#`) and produce a key the serialized data
    // does not carry.
    let field_name = ident.unraw().to_string();
    Ok(rename_all
        .map(|rule| rule.apply(&field_name))
        .unwrap_or(field_name))
}

/// Returns true if the field has `#[serde(default)]` or `#[serde(default = ...)]`.
pub(crate) fn serde_has_default(attrs: &[Attribute]) -> syn::Result<bool> {
    let mut has_default = false;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if parse_default_meta(&meta)? {
                has_default = true;
            }
            Ok(())
        })?;
    }
    Ok(has_default)
}

/// Recognizes `default`, consuming an optional expression so nested parsing
/// remains positioned at the next Serde attribute.
fn parse_default_meta(meta: &ParseNestedMeta) -> syn::Result<bool> {
    if !meta.path.is_ident("default") {
        super::discard_unknown(meta)?;
        return Ok(false);
    }

    if meta.input.peek(Token![=]) {
        meta.value()?.parse::<syn::Expr>()?;
    }
    Ok(true)
}
