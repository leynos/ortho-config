//! Struct-level `#[ortho_config(...)]` attribute parsing.

use syn::meta::ParseNestedMeta;
use syn::parenthesized;
use syn::{Attribute, Lit, Token};

use super::doc_attrs::apply_struct_doc_attr;
use super::literals::{lit_bool, lit_char, lit_crate_path, lit_str};
use super::{DiscoveryAttrs, StructAttrs};

pub(crate) fn parse_ortho_config<F>(attrs: &[Attribute], mut f: F) -> syn::Result<()>
where
    F: FnMut(&syn::meta::ParseNestedMeta) -> syn::Result<()>,
{
    for attr in attrs.iter().filter(|a| a.path().is_ident("ortho_config")) {
        attr.parse_nested_meta(|meta| f(&meta))?;
    }
    Ok(())
}

/// Consumes an unrecognised key-value or list without recording it.
pub(crate) fn discard_unknown(meta: &syn::meta::ParseNestedMeta) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        meta.value()?.parse::<proc_macro2::TokenStream>()?;
    } else if meta.input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in meta.input);
        content.parse::<proc_macro2::TokenStream>()?;
    }
    Ok(())
}

pub(crate) fn parse_prefix(meta: &ParseNestedMeta) -> syn::Result<String> {
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
            Some("profiles") => {
                // Accept both `profiles` and `profiles = true`
                let v = if meta.input.peek(Token![=]) {
                    meta.value()?.parse::<syn::LitBool>()?.value
                } else {
                    true
                };
                out.profiles = v;
                Ok(())
            }
            Some("crate") => {
                if out.crate_path.is_some() {
                    return Err(syn::Error::new_spanned(
                        &meta.path,
                        "duplicate `crate` attribute",
                    ));
                }
                out.crate_path = Some(lit_crate_path(meta)?);
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
