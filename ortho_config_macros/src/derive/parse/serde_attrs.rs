//! Serde attribute parsing helpers.
//!
//! The derive macro uses these utilities to compute the JSON keys serde will
//! emit when serializing the parsed CLI struct. This is required for
//! `cli_default_as_absent` extraction: we serialize `self` to JSON and then
//! pluck individual fields out of the resulting object, so the lookup key must
//! respect `#[serde(rename = "...")]` and `#[serde(rename_all = "...")]`.

use heck::{
    ToKebabCase, ToLowerCamelCase, ToShoutyKebabCase, ToShoutySnakeCase, ToSnakeCase,
    ToUpperCamelCase,
};
use syn::meta::ParseNestedMeta;
use syn::{Attribute, Expr, Field, LitStr, Token};

/// Supported `#[serde(rename_all = "...")]` rules for struct fields.
///
/// This is used by the `cli_default_as_absent` extraction implementation to
/// compute the same JSON field names that serde uses during serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SerdeRenameAll {
    Lower,
    Upper,
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

impl SerdeRenameAll {
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

    fn apply(self, field_name: &str) -> String {
        match self {
            Self::Lower => field_name.to_ascii_lowercase(),
            Self::Upper => field_name.to_ascii_uppercase(),
            Self::Pascal => field_name.to_upper_camel_case(),
            Self::Camel => field_name.to_lower_camel_case(),
            Self::Snake => field_name.to_snake_case(),
            Self::ScreamingSnake => field_name.to_shouty_snake_case(),
            Self::Kebab => field_name.to_kebab_case(),
            Self::ScreamingKebab => field_name.to_shouty_kebab_case(),
        }
    }
}

/// Parse `#[serde(rename_all = "...")]` from struct attributes.
pub(crate) fn serde_rename_all(attrs: &[Attribute]) -> syn::Result<Option<SerdeRenameAll>> {
    let mut out = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                let value = meta.value()?.parse::<LitStr>()?;
                out = Some(SerdeRenameAll::parse(&value)?);
            } else {
                super::discard_unknown(&meta)?;
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// Parse `#[serde(rename = "...")]` (and `rename(serialize = "...")`) from field attributes.
pub(crate) fn serde_field_rename(attrs: &[Attribute]) -> syn::Result<Option<String>> {
    let mut out = None;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("rename") {
                super::discard_unknown(&meta)?;
                return Ok(());
            }

            if meta.input.peek(Token![=]) {
                let value = meta.value()?.parse::<LitStr>()?;
                out = Some(value.value());
                return Ok(());
            }

            if !meta.input.peek(syn::token::Paren) {
                return Ok(());
            }

            meta.parse_nested_meta(|nested| parse_serde_rename_serialize(&nested, &mut out))?;
            Ok(())
        })?;
    }
    Ok(out)
}

fn parse_serde_rename_serialize(
    nested: &ParseNestedMeta,
    rename: &mut Option<String>,
) -> syn::Result<()> {
    if !nested.path.is_ident("serialize") {
        super::discard_unknown(nested)?;
        return Ok(());
    }

    let value = nested.value()?.parse::<LitStr>()?;
    *rename = Some(value.value());
    Ok(())
}

/// Compute the JSON key serde uses for `field` given an optional container-level rename rule.
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
    let field_name = ident.to_string();
    Ok(rename_all
        .map(|rule| rule.apply(&field_name))
        .unwrap_or(field_name))
}

/// Returns true if the field has `#[serde(default)]` or `#[serde(default = ...)]`.
pub(crate) fn serde_has_default(attrs: &[Attribute]) -> syn::Result<bool> {
    let mut has_default = false;
    for attr in attrs.iter().filter(|attr| attr.path().is_ident("serde")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("default") {
                has_default = true;
                if meta.input.peek(Token![=]) {
                    meta.value()?.parse::<syn::Expr>()?;
                }
                return Ok(());
            }
            super::discard_unknown(&meta)
        })?;
    }
    Ok(has_default)
}
