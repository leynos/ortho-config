//! Parsing utilities for the `OrthoConfig` derive macro.
//!
//! Basic compile-check example:
//!
//! ```rust
//! // This trivial example exists to keep doctests compiling in this module.
//! // The parsing helpers below are internal to the macro and exercised by
//! // unit tests; this snippet simply guards against accidental doctest
//! // breakage (e.g., invalid code fences).
//! let _ = 1 + 1;
//! ```

use syn::meta::ParseNestedMeta;
use syn::parenthesized;
use syn::{Attribute, Expr, Lit, LitStr, Token};

mod clap_attrs;
mod input;
mod serde_attrs;
#[cfg(test)]
mod tests;
mod type_utils;

pub(crate) use clap_attrs::{clap_arg_id, clap_arg_id_from_attribute};
pub(crate) use input::parse_input;
pub(crate) use serde_attrs::{
    SerdeRenameAll, serde_field_rename, serde_rename_all, serde_serialized_field_key,
};
pub(crate) use type_utils::{btree_map_inner, option_inner, vec_inner};

const _: fn(&Attribute, &mut Option<LitStr>) -> syn::Result<()> = clap_arg_id_from_attribute;
const _: fn(&[Attribute]) -> syn::Result<Option<String>> = serde_field_rename;

#[derive(Default, Clone)]
pub(crate) struct StructAttrs {
    pub prefix: Option<String>,
    pub discovery: Option<DiscoveryAttrs>,
}

/// Field-level attributes recognised by `#[derive(OrthoConfig)]`.
///
/// - `cli_long`/`cli_short` override generated CLI flags.
/// - `default` supplies a compile-time default expression when no layer
///   configures the field.
/// - `merge_strategy` selects how collections combine during declarative
///   merges.
/// - `skip_cli` omits the field from CLI parsing whilst leaving declarative
///   merging untouched.
/// - `cli_default_as_absent` treats clap's default value as absent during
///   merge, allowing file/env values to take precedence over CLI defaults.
#[derive(Default, Clone)]
pub(crate) struct FieldAttrs {
    pub cli_long: Option<String>,
    pub cli_short: Option<char>,
    pub default: Option<Expr>,
    pub merge_strategy: Option<MergeStrategy>,
    pub skip_cli: bool,
    pub cli_default_as_absent: bool,
}

#[derive(Default, Clone)]
pub(crate) struct DiscoveryAttrs {
    pub app_name: Option<String>,
    pub env_var: Option<String>,
    pub config_file_name: Option<String>,
    pub dotfile_name: Option<String>,
    pub project_file_name: Option<String>,
    pub config_cli_long: Option<String>,
    pub config_cli_short: Option<char>,
    pub config_cli_visible: Option<bool>,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MergeStrategy {
    Append,
    Replace,
    Keyed,
}

impl MergeStrategy {
    pub(crate) fn parse(s: &str, span: proc_macro2::Span) -> Result<Self, syn::Error> {
        match s {
            "append" => Ok(Self::Append),
            "replace" => Ok(Self::Replace),
            "keyed" => Ok(Self::Keyed),
            _ => Err(syn::Error::new(
                span,
                format!(
                    "unknown merge_strategy '{s}'; expected one of \"append\", \"replace\", or \"keyed\""
                ),
            )),
        }
    }
}

/// Iterate all `#[ortho_config(...)]` attributes once and apply a callback.
fn parse_ortho_config<F>(attrs: &[Attribute], mut f: F) -> syn::Result<()>
where
    F: FnMut(&syn::meta::ParseNestedMeta) -> syn::Result<()>,
{
    for attr in attrs.iter().filter(|a| a.path().is_ident("ortho_config")) {
        attr.parse_nested_meta(|meta| f(&meta))?;
    }
    Ok(())
}

/// Consumes an unrecognised key-value or list without recording it.
fn discard_unknown(meta: &syn::meta::ParseNestedMeta) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        meta.value()?.parse::<proc_macro2::TokenStream>()?;
    } else if meta.input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in meta.input);
        content.parse::<proc_macro2::TokenStream>()?;
    }
    Ok(())
}

fn parse_prefix(meta: &ParseNestedMeta) -> syn::Result<String> {
    let lit = meta.value()?.parse::<Lit>()?;
    match lit {
        Lit::Str(s) => {
            let mut value = s.value();
            if !value.is_empty() && !value.ends_with('_') {
                value.push('_');
            }
            Ok(value)
        }
        other => Err(syn::Error::new(other.span(), "prefix must be a string")),
    }
}

fn parse_discovery_meta(meta: &ParseNestedMeta, discovery: &mut DiscoveryAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| handle_discovery_nested(&nested, discovery))
}

fn handle_discovery_nested(
    nested: &ParseNestedMeta,
    discovery: &mut DiscoveryAttrs,
) -> syn::Result<()> {
    let Some(ident) = nested.path.get_ident().map(ToString::to_string) else {
        return discard_unknown(nested);
    };

    match ident.as_str() {
        "app_name" => assign_str(&mut discovery.app_name, nested, "app_name"),
        "env_var" => assign_str(&mut discovery.env_var, nested, "env_var"),
        "config_file_name" => {
            assign_str(&mut discovery.config_file_name, nested, "config_file_name")
        }
        "dotfile_name" => assign_str(&mut discovery.dotfile_name, nested, "dotfile_name"),
        "project_file_name" => assign_str(
            &mut discovery.project_file_name,
            nested,
            "project_file_name",
        ),
        "config_cli_long" => assign_str(&mut discovery.config_cli_long, nested, "config_cli_long"),
        "config_cli_short" => {
            assign_char(&mut discovery.config_cli_short, nested, "config_cli_short")
        }
        "config_cli_visible" => assign_bool(
            &mut discovery.config_cli_visible,
            nested,
            "config_cli_visible",
        ),
        _ => discard_unknown(nested),
    }
}

fn assign_str(target: &mut Option<String>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_str(nested, key)?.value();
    *target = Some(value);
    Ok(())
}

fn assign_char(target: &mut Option<char>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_char(nested, key)?;
    *target = Some(value);
    Ok(())
}

fn assign_bool(target: &mut Option<bool>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_bool(nested, key)?;
    *target = Some(value);
    Ok(())
}

/// Extracts `#[ortho_config(...)]` metadata applied to a struct.
///
/// Only the `prefix` key is currently recognised. Unknown keys are
/// ignored so callers keep compiling when new attributes appear. This
/// improves forwards compatibility at the cost of allowing silent typos.
/// If stricter validation is desired, a custom `compile_error!` guard can
/// reject unexpected keys.
///
/// Used internally by the derive macro to extract configuration metadata
/// from struct-level attributes.
pub(crate) fn parse_struct_attrs(attrs: &[Attribute]) -> Result<StructAttrs, syn::Error> {
    let mut out = StructAttrs::default();
    parse_ortho_config(attrs, |meta| {
        match meta.path.get_ident().map(ToString::to_string).as_deref() {
            Some("prefix") => {
                let value = parse_prefix(meta)?;
                out.prefix = Some(value);
                Ok(())
            }
            Some("discovery") => {
                let mut discovery = out.discovery.take().unwrap_or_default();
                parse_discovery_meta(meta, &mut discovery)?;
                out.discovery = Some(discovery);
                Ok(())
            }
            _ => discard_unknown(meta),
        }
    })?;
    Ok(out)
}

/// Parses a literal from a field attribute using `extractor`.
///
/// # Examples
///
/// ```ignore
/// # use syn::meta::ParseNestedMeta;
/// # use syn::{Lit, LitStr};
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let s: LitStr = parse_lit(meta, "cli_long", |lit| match lit {
///     Lit::Str(s) => Some(s),
///     _ => None,
/// })?;
/// # Ok(())
/// # }
/// ```
fn parse_lit<T, F>(
    meta: &syn::meta::ParseNestedMeta,
    key: &str,
    extractor: F,
) -> Result<T, syn::Error>
where
    F: FnOnce(Lit) -> Option<T>,
{
    let literal = meta.value()?.parse::<Lit>()?;
    let span = literal.span();
    extractor(literal).ok_or_else(|| {
        let type_name = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .unwrap_or("literal")
            .to_lowercase();
        let display_type = match type_name.as_str() {
            "litstr" => "string",
            other => other,
        };
        syn::Error::new(span, format!("{key} must be a {display_type}"))
    })
}

/// Parses a string literal from a field attribute.
///
/// # Examples
///
/// ```rust,ignore
/// // Build a synthetic attribute and visit its nested meta so we can call into
/// // the parsing helper in this crate. The nightly-2025-09-16 toolchain that
/// // backs this repository currently ICEs when compiling the snippet, so the
/// // example is marked `ignore` until the regression is fixed.
/// use syn::Attribute;
/// let attr: Attribute = syn::parse_quote!(#[ortho_config(cli_long = "name")]);
/// attr.parse_nested_meta(|meta| {
///     let s = ortho_config_macros::__doc_lit_str(&meta, "cli_long")?;
///     assert_eq!(s.value(), "name");
///     Ok(())
/// }).unwrap();
/// ```
fn lit_str(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<LitStr, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Str(s) => Some(s),
        _ => None,
    })
}

/// Parses a character literal from a field attribute.
///
/// # Examples
///
/// ```rust,ignore
/// # use syn::meta::ParseNestedMeta;
/// # fn demo(meta: &ParseNestedMeta) -> syn::Result<()> {
/// let c = lit_char(meta, "cli_short")?;
/// assert_eq!(c, 'n');
/// # Ok(())
/// # }
/// ```
fn lit_char(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<char, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Char(c) => Some(c.value()),
        _ => None,
    })
}

fn lit_bool(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<bool, syn::Error> {
    parse_lit(meta, key, |lit| match lit {
        Lit::Bool(b) => Some(b.value),
        _ => None,
    })
}

/// Applies a recognised field attribute, returning `true` if handled.
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
    match () {
        () if meta.path.is_ident("cli_long") => {
            let s = lit_str(meta, "cli_long")?;
            out.cli_long = Some(s.value());
            Ok(true)
        }
        () if meta.path.is_ident("cli_short") => {
            let c = lit_char(meta, "cli_short")?;
            out.cli_short = Some(c);
            Ok(true)
        }
        () if meta.path.is_ident("default") => {
            out.default = Some(meta.value()?.parse()?);
            Ok(true)
        }
        () if meta.path.is_ident("merge_strategy") => {
            let s = lit_str(meta, "merge_strategy")?;
            out.merge_strategy = Some(MergeStrategy::parse(&s.value(), s.span())?);
            Ok(true)
        }
        () if meta.path.is_ident("skip_cli") => {
            out.skip_cli = true;
            Ok(true)
        }
        () if meta.path.is_ident("cli_default_as_absent") => {
            out.cli_default_as_absent = true;
            Ok(true)
        }
        () => Ok(false),
    }
}

// Expose a thin wrapper for doctests without leaking internals into the public
// API in normal builds. This allows examples to type-check while keeping
// `lit_str` private outside of tests/doctests.
#[cfg(any(test, doctest))]
#[doc(hidden)]
pub fn __doc_lit_str(meta: &syn::meta::ParseNestedMeta, key: &str) -> Result<LitStr, syn::Error> {
    lit_str(meta, key)
}

/// Parses field-level `#[ortho_config(...)]` attributes.
///
/// Recognised keys include `cli_long`, `cli_short`, `default`,
/// `merge_strategy`, and `skip_cli`. Unknown keys are ignored, matching
/// [`parse_struct_attrs`] for forwards compatibility. This lenience may
/// permit misspelt attribute names; users wanting stricter validation can
/// insert a manual `compile_error!` guard.
///
/// Used internally by the derive macro to extract configuration metadata
/// from field-level attributes.
pub(crate) fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs, syn::Error> {
    let mut out = FieldAttrs::default();
    parse_ortho_config(attrs, |meta| {
        if !apply_field_attr(meta, &mut out)? {
            // Unknown attributes are intentionally discarded to preserve
            // forwards compatibility while still allowing callers to add
            // new keys in future versions.
            discard_unknown(meta)?;
        }
        Ok(())
    })?;
    Ok(out)
}
