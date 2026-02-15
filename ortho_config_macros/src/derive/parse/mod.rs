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
mod doc_attrs;
mod doc_types;
mod input;
mod literals;
mod serde_attrs;
#[cfg(test)]
mod tests;
mod type_utils;

use clap_attrs::clap_default_value;
pub(crate) use clap_attrs::{ClapInferredDefault, clap_arg_id, clap_arg_id_from_attribute};
use doc_attrs::{apply_field_doc_attr, apply_struct_doc_attr};
pub(crate) use doc_types::{
    DocExampleAttr, DocFieldAttrs, DocLinkAttr, DocNoteAttr, DocStructAttrs, HeadingOverrides,
};
pub(crate) use input::parse_input;
#[cfg(any(test, doctest))]
pub(crate) use literals::__doc_lit_str;
use literals::{lit_bool, lit_char, lit_str};
pub(crate) use serde_attrs::{
    SerdeRenameAll, serde_field_rename, serde_has_default, serde_rename_all,
    serde_serialized_field_key,
};
pub(crate) use type_utils::{btree_map_inner, hash_map_inner, option_inner, vec_inner};

const _: fn(&Attribute, &mut Option<LitStr>) -> syn::Result<()> = clap_arg_id_from_attribute;
const _: fn(&[Attribute]) -> syn::Result<Option<String>> = serde_field_rename;

#[derive(Default, Clone)]
pub(crate) struct StructAttrs {
    pub prefix: Option<String>,
    pub discovery: Option<DiscoveryAttrs>,
    pub post_merge_hook: bool,
    pub doc: DocStructAttrs,
    /// Overrides the generated crate path for dependency aliasing.
    ///
    /// When set via `#[ortho_config(crate = "my_alias")]`, generated code
    /// references types through `my_alias::` instead of `ortho_config::`.
    pub crate_path: Option<syn::Path>,
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
/// - `inferred_clap_default` stores the default inferred from clap's
///   `default_value_t`/`default_values_t` when `cli_default_as_absent` is
///   active and no explicit `#[ortho_config(default = ...)]` is provided.
#[derive(Default, Clone)]
pub(crate) struct FieldAttrs {
    pub cli_long: Option<String>,
    pub cli_short: Option<char>,
    pub default: Option<Expr>,
    pub inferred_clap_default: Option<ClapInferredDefault>,
    pub merge_strategy: Option<MergeStrategy>,
    pub skip_cli: bool,
    pub cli_default_as_absent: bool,
    pub doc: DocFieldAttrs,
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
            Some("post_merge_hook") => {
                // Accept both `post_merge_hook` and `post_merge_hook = true`
                let v = if meta.input.peek(Token![=]) {
                    meta.value()?.parse::<syn::LitBool>()?.value
                } else {
                    true
                };
                out.post_merge_hook = v;
                Ok(())
            }
            Some("crate") => {
                let s = lit_str(meta, "crate")?;
                let path: syn::Path =
                    syn::parse_str(&s.value()).map_err(|e| syn::Error::new(s.span(), e))?;
                out.crate_path = Some(path);
                Ok(())
            }
            _ => {
                if apply_struct_doc_attr(meta, &mut out.doc)? {
                    return Ok(());
                }
                discard_unknown(meta)
            }
        }
    })?;
    Ok(out)
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
    let Some(ident) = meta.path.get_ident() else {
        return Ok(false);
    };
    let key = ident.to_string();
    match key.as_str() {
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
            let v = if meta.input.peek(Token![=]) {
                meta.value()?.parse::<syn::LitBool>()?.value
            } else {
                true
            };
            out.cli_default_as_absent = v;
            Ok(true)
        }
        _ => apply_field_doc_attr(meta, out),
    }
}

/// Parses field-level `#[ortho_config(...)]` attributes.
///
/// Recognised keys include `cli_long`, `cli_short`, `default`,
/// `merge_strategy`, `skip_cli`, and `cli_default_as_absent`. Unknown keys are
/// ignored, matching [`parse_struct_attrs`] for forwards compatibility. This
/// lenience may permit misspelt attribute names; users wanting stricter
/// validation can insert a manual `compile_error!` guard.
///
/// When `cli_default_as_absent` is active and no explicit `default` is
/// provided, this function attempts to infer a default from clap's
/// `default_value_t` or `default_values_t`. Inference from the untyped
/// `default_value` is rejected with a compile-time error.
///
/// Used internally by the derive macro to extract configuration metadata
/// from field-level attributes.
pub(crate) fn parse_field_attrs(field: &syn::Field) -> Result<FieldAttrs, syn::Error> {
    let mut out = FieldAttrs::default();
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
        if let Some(ClapInferredDefault::Value(_)) = out.inferred_clap_default {
            return Err(syn::Error::new_spanned(
                field,
                concat!(
                    "inferring defaults from clap `default_value` is not yet supported for ",
                    "`cli_default_as_absent`; use `default_value_t`/`default_values_t` or ",
                    "add `#[ortho_config(default = ...)]`. Parser-faithful `default_value` ",
                    "inference is planned as a day-2 follow-up."
                ),
            ));
        }
    }
    Ok(out)
}
