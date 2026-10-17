//! Struct-level `#[ortho_config(...)]` attribute parsing.

use syn::meta::ParseNestedMeta;
use syn::parenthesized;
use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Lit, LitStr, Token};

use super::doc_attrs::apply_struct_doc_attr;
use super::literals::{lit_bool, lit_char, lit_crate_path, lit_str};
use super::localization;
use super::{DiscoveryAttrs, StructAttrs, parse_prefix};

/// Visits nested entries from each `#[ortho_config(...)]` attribute in source order.
///
/// The callback owns key interpretation; syntax errors propagate from `syn`.
///
/// # Examples
///
/// Given `#[ortho_config(prefix = "APP")]`, the callback receives the nested
/// `prefix` entry; malformed nested syntax is returned as a `syn` error.
pub(crate) fn parse_ortho_config<F>(attrs: &[Attribute], mut f: F) -> syn::Result<()>
where
    F: FnMut(&syn::meta::ParseNestedMeta) -> syn::Result<()>,
{
    for attr in attrs.iter().filter(|a| a.path().is_ident("ortho_config")) {
        attr.parse_nested_meta(|meta| f(&meta))?;
    }
    Ok(())
}

/// Consumes an unrecognized key-value or list without recording it.
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

/// Parses nested discovery settings into the shared discovery model.
///
/// # Examples
///
/// A nested `app_name = "editor"` entry updates `DiscoveryAttrs::app_name`
/// while preserving the other discovery settings.
fn parse_discovery_meta(meta: &ParseNestedMeta, discovery: &mut DiscoveryAttrs) -> syn::Result<()> {
    meta.parse_nested_meta(|nested| handle_discovery_nested(&nested, discovery))
}

/// Assigns one recognized discovery key, consuming unknown keys compatibly.
///
/// # Examples
///
/// `app_name = "editor"` updates the model; an unknown key is consumed
/// without changing it so future attributes remain compatible.
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
        "env_vars" => assign_string_list(&mut discovery.env_vars, nested, "env_vars"),
        "explicit_mode" => assign_str(&mut discovery.explicit_mode, nested, "explicit_mode"),
        "automatic_mode" => assign_str(&mut discovery.automatic_mode, nested, "automatic_mode"),
        "scope_order" => assign_string_list(&mut discovery.scope_order, nested, "scope_order"),
        "project_root_from" => assign_str(
            &mut discovery.project_root_from,
            nested,
            "project_root_from",
        ),
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

/// Replaces a list-valued discovery setting from a string array or parenthesized list.
///
/// Non-string array elements are rejected at the offending expression span.
///
/// # Examples
///
/// `env_vars = ["APP_CONFIG", "APP_SETTINGS"]` stores both names in order; a
/// non-string array element returns a diagnostic at that element.
fn assign_string_list(
    target: &mut Vec<String>,
    nested: &ParseNestedMeta,
    key: &str,
) -> syn::Result<()> {
    let values = if nested.input.peek(Token![=]) {
        let array = nested.value()?.parse::<syn::ExprArray>()?;
        array
            .elems
            .into_iter()
            .map(|expression| match expression {
                Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(value),
                    ..
                }) => Ok(value.value()),
                other => Err(syn::Error::new_spanned(
                    other,
                    format!("{key} must contain strings"),
                )),
            })
            .collect::<syn::Result<Vec<_>>>()?
    } else {
        let content;
        parenthesized!(content in nested.input);
        Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .map(|value| value.value())
            .collect()
    };
    *target = values;
    Ok(())
}

/// Parses and replaces one string-valued discovery setting.
///
/// # Examples
///
/// `config_file_name = "settings.toml"` stores that name in the target slot.
fn assign_str(target: &mut Option<String>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_str(nested, key)?.value();
    *target = Some(value);
    Ok(())
}

/// Parses and replaces one character-valued discovery setting.
///
/// # Examples
///
/// `config_cli_short = 'c'` stores `Some('c')` for the generated option.
fn assign_char(target: &mut Option<char>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_char(nested, key)?;
    *target = Some(value);
    Ok(())
}

/// Parses and replaces one boolean-valued discovery setting.
///
/// # Examples
///
/// `config_cli_visible = false` stores `Some(false)`, preserving an explicit
/// request to hide the option from help.
fn assign_bool(target: &mut Option<bool>, nested: &ParseNestedMeta, key: &str) -> syn::Result<()> {
    let value = lit_bool(nested, key)?;
    *target = Some(value);
    Ok(())
}

/// Extracts `#[ortho_config(...)]` metadata applied to a struct.
///
/// Recognizes `prefix`, `discovery`, `post_merge_hook`, `profiles`, `crate`,
/// and the struct-level documentation keys. Unknown keys are
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
                if localization::parse_localization_attr(meta, &mut out)? {
                    return Ok(());
                }
                if apply_struct_doc_attr(meta, &mut out.doc)? {
                    return Ok(());
                }
                discard_unknown(meta)
            }
        }
    })?;
    if out
        .discovery
        .as_ref()
        .is_some_and(has_conflicting_env_selectors)
    {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "`env_var` and `env_vars` are mutually exclusive",
        ));
    }
    Ok(out)
}

/// Detects mutually exclusive single-name and ordered-list environment selectors.
///
/// # Examples
///
/// A discovery setting with only `env_var` returns `false`; setting both
/// `env_var` and a non-empty `env_vars` list returns `true` and is rejected.
const fn has_conflicting_env_selectors(discovery: &DiscoveryAttrs) -> bool {
    discovery.env_var.is_some() && !discovery.env_vars.is_empty()
}
