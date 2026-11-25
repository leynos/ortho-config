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

/// Consumes an unrecognised key-value or list without recording it.
///
/// A name-value item must consume only its own value. Parsing a
/// `TokenStream` from `meta.value()` would swallow the rest of the nested
/// list, because that parser consumes everything it is given, so a later
/// `flatten` in `#[serde(rename = "common", flatten)]` would never reach the
/// enclosing callback. Reading one token tree at a time and stopping at a
/// top-level comma leaves the enclosing `parse_nested_meta` loop positioned
/// on the next item. A group token tree carries its own delimiters, so a comma
/// inside parentheses or brackets cannot end the value early, and the scan
/// tracks a turbofish's angle brackets so that a comma in `factory::<A, B>()`
/// is not mistaken for one either.
pub(crate) fn discard_unknown(meta: &syn::meta::ParseNestedMeta) -> syn::Result<()> {
    if meta.input.peek(Token![=]) {
        skip_to_item_end(meta.value()?)?;
    } else if meta.input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in meta.input);
        content.parse::<proc_macro2::TokenStream>()?;
    }
    Ok(())
}

/// Tracks the nesting a token scan must respect.
///
/// An item boundary is a comma that is not enclosed by anything. Two constructs
/// carry top-level commas without being delimiter groups, so both are counted
/// here:
///
/// - a turbofish: a `<` that follows `::` opens a level, a `<` already inside a
///   level opens a nested one (`outer::<Inner<A, B>, C>()`), and a `>` closes
///   one — except after `-`, so the arrow in `dyn Fn() -> T` cannot;
/// - a closure's parameter list: `|a, b| a` puts its comma at top level.
///
/// A closure bar is recognised only while the value is still in its prefix, so
/// a later `|` stays bitwise or (`a | b`). The prefix is not just the first
/// token: `move |a, b| a` puts the `move` keyword before the bar, and
/// `async move` may precede it too, so those keywords keep the prefix open.
/// The previous `syn::Expr` parse accepted every one of these shapes.
#[derive(Default)]
struct ItemNesting {
    /// How many turbofish argument lists are open.
    angle_depth: usize,
    /// Consecutive `:` tokens seen, to recognise the `::<` opener.
    colons: u8,
    /// Whether the previous token was `-`, to recognise an arrow's `>`.
    after_dash: bool,
    /// Whether a closure's parameter list is open.
    inside_closure_params: bool,
    /// Whether only closure-prefix keywords have been consumed so far.
    at_value_prefix: bool,
}

/// Keywords that may precede a closure's parameter list.
///
/// `move` and `static` capture by move or by reference for `'static`; `async`
/// marks an async block or closure. All three are ordinary identifiers to the
/// token stream, so the scan compares their spelling.
const CLOSURE_PREFIX_KEYWORDS: [&str; 3] = ["move", "async", "static"];

impl ItemNesting {
    /// Reports whether a comma here ends the value rather than nesting.
    const fn at_top_level(&self) -> bool {
        self.angle_depth == 0 && !self.inside_closure_params
    }

    /// Updates the nesting from one token.
    fn observe(&mut self, tree: &proc_macro2::TokenTree) {
        if self.at_value_prefix && is_closure_prefix_keyword(tree) {
            return;
        }
        let ch = match tree {
            proc_macro2::TokenTree::Punct(punct) => Some(punct.as_char()),
            _ => None,
        };
        let opens_bar = ch == Some('|') && self.at_value_prefix;
        if opens_bar {
            // `||` arrives as two bars: the first opens the parameter list and
            // the second closes it, which is exactly what every zero-parameter
            // closure needs.
            self.inside_closure_params = true;
            self.colons = 0;
        } else if ch == Some('|') && self.inside_closure_params {
            self.inside_closure_params = false;
            self.colons = 0;
        } else {
            self.observe_angle(ch);
        }
        self.after_dash = ch == Some('-');
        self.at_value_prefix = false;
    }

    /// Applies the turbofish rules to a token that is not a closure bar.
    const fn observe_angle(&mut self, ch: Option<char>) {
        match ch {
            Some(':') => self.colons = self.colons.saturating_add(1),
            Some('<') if self.angle_depth > 0 || self.colons >= 2 => {
                self.angle_depth += 1;
                self.colons = 0;
            }
            Some('>') if self.after_dash => self.colons = 0,
            Some('>') if self.angle_depth > 0 => {
                self.angle_depth -= 1;
                self.colons = 0;
            }
            _ => self.colons = 0,
        }
    }
}

/// Reports whether a token is a keyword that may precede a closure's bars.
///
/// The token stream carries `move` as an identifier, so its spelling is the
/// only way to recognise it.
fn is_closure_prefix_keyword(tree: &proc_macro2::TokenTree) -> bool {
    match tree {
        proc_macro2::TokenTree::Ident(ident) => {
            CLOSURE_PREFIX_KEYWORDS.contains(&ident.to_string().as_str())
        }
        _ => false,
    }
}

/// Consumes tokens up to the next item separator, leaving the comma unread.
///
/// Reading one token tree at a time and stopping at a top-level comma keeps
/// the enclosing `parse_nested_meta` loop positioned on the next item, so a
/// later `flatten` in `#[serde(rename = "common", flatten)]` still reaches its
/// handler. The nesting rules that make "top-level" meaningful are in
/// [`ItemNesting`].
fn skip_to_item_end(input: syn::parse::ParseStream<'_>) -> syn::Result<()> {
    let mut nesting = ItemNesting {
        at_value_prefix: true,
        ..ItemNesting::default()
    };
    while !input.is_empty() {
        if nesting.at_top_level() && input.peek(Token![,]) {
            break;
        }
        let tree: proc_macro2::TokenTree = input.parse()?;
        nesting.observe(&tree);
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
