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
mod discovery;
mod doc_attrs;
mod doc_types;
mod input;
mod literals;
mod serde_attrs;
#[cfg(test)]
mod tests;
mod type_utils;

pub(crate) use clap_attrs::{
    ClapDefaultValue, ClapInferredDefault, clap_arg_id, clap_arg_id_from_attribute,
    clap_default_value, clap_field_is_subcommand, clap_variant_name,
    reject_subcommand_ortho_config_attrs,
};
use discovery::parse_discovery_meta;
use doc_attrs::{apply_field_doc_attr, apply_struct_doc_attr};
pub(crate) use doc_types::{
    DocExampleAttr, DocFieldAttrs, DocLinkAttr, DocNoteAttr, DocStructAttrs, HeadingOverrides,
};
pub(crate) use input::parse_input;
#[cfg(any(test, doctest))]
pub(crate) use literals::__doc_lit_str;
pub(crate) use literals::lit_crate_path;
use literals::{lit_char, lit_str};
pub(crate) use serde_attrs::{
    SerdeRenameAll, serde_field_rename, serde_has_default, serde_rename_all,
    serde_serialized_field_key,
};
pub(crate) use type_utils::{
    ClapDefaultValueShape, btree_map_inner, hash_map_inner, option_inner, vec_inner,
};

const _: fn(&Attribute, &mut Option<LitStr>) -> syn::Result<()> = clap_arg_id_from_attribute;
const _: fn(&syn::Field) -> syn::Result<bool> = clap_field_is_subcommand;
const _: fn(&syn::Variant) -> syn::Result<Option<LitStr>> = clap_variant_name;
const _: fn(&[Attribute]) -> syn::Result<Option<String>> = serde_field_rename;

/// Parsed options attached to the configuration type; these drive generated
/// discovery and loading while keeping documentation metadata alongside them.
#[derive(Default, Clone)]
pub(crate) struct StructAttrs {
    /// Prefix applied to environment variable names, normalized with a trailing underscore.
    pub prefix: Option<String>,
    /// Optional names and switches that configure platform-aware file discovery.
    pub discovery: Option<DiscoveryAttrs>,
    /// Whether generated loading invokes the post-merge hook after composition.
    pub post_merge_hook: bool,
    /// Documentation metadata copied into generated API documentation.
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
/// - `is_subcommand` marks a clap subcommand selector, which is excluded from
///   configuration-field generation.
/// - `inferred_clap_default` stores the default inferred from clap's
///   `default_value`, `default_value_t`, or `default_values_t` when
///   `cli_default_as_absent` is
///   active and no explicit `#[ortho_config(default = ...)]` is provided.
#[derive(Default, Clone)]
pub(crate) struct FieldAttrs {
    /// Explicit long option name, used instead of the name inferred by clap.
    pub cli_long: Option<String>,
    /// Explicit short option name, used instead of the name inferred by clap.
    pub cli_short: Option<char>,
    /// User expression supplying the field value when no source configures it.
    pub default: Option<Expr>,
    /// Clap-derived default retained when a CLI default must be treated as absent.
    pub inferred_clap_default: Option<ClapInferredDefault>,
    /// Collection merge behaviour to use when composing configuration layers.
    pub merge_strategy: Option<MergeStrategy>,
    /// Whether the field is omitted from CLI generation while remaining mergeable.
    pub skip_cli: bool,
    /// Whether a clap-provided default is omitted from the CLI merge layer.
    pub cli_default_as_absent: bool,
    /// Whether clap owns the field as a subcommand selector rather than config data.
    pub is_subcommand: bool,
    /// Documentation metadata emitted for the generated field API.
    pub doc: DocFieldAttrs,
}

/// Parsed names and switches for locating configuration files before merging.
#[derive(Default, Clone)]
pub(crate) struct DiscoveryAttrs {
    /// Application identity used to derive standard discovery locations.
    pub app_name: Option<String>,
    /// Environment variable whose value can name a configuration file.
    pub env_var: Option<String>,
    /// Explicit configuration filename searched by the discovery builder.
    pub config_file_name: Option<String>,
    /// Explicit per-user dotfile name used by discovery.
    pub dotfile_name: Option<String>,
    /// Explicit project-level filename used by discovery.
    pub project_file_name: Option<String>,
    /// Long CLI option that selects an explicit configuration path.
    pub config_cli_long: Option<String>,
    /// Short CLI option that selects an explicit configuration path.
    pub config_cli_short: Option<char>,
    /// Whether the generated config-path option is shown in CLI help.
    pub config_cli_visible: Option<bool>,
}

/// Collection merge policy selected by a field's `merge_strategy` attribute.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum MergeStrategy {
    /// Add sequence values from each higher-precedence layer to the prior values.
    Append,
    /// Replace the prior value with the value from the higher-precedence layer.
    Replace,
    /// Merge map entries by key, retaining unaffected entries from lower layers.
    Keyed,
}

impl MergeStrategy {
    /// Parse the accepted attribute spelling, reporting unknown values at their source span.
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

/// Parse a string prefix and normalize non-empty values for environment names.
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

/// Applies one recognized field option, returning `false` for unknown keys.
///
/// Parsing errors are returned at the attribute site; successful assignments
/// replace the corresponding slot in `out` before the next nested key is read.
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
/// Parse the optional boolean value of `cli_default_as_absent`.
///
/// A bare `cli_default_as_absent` enables the behaviour; an explicit
/// `cli_default_as_absent = <bool>` sets it directly.
fn parse_cli_default_as_absent(meta: &syn::meta::ParseNestedMeta) -> Result<bool, syn::Error> {
    if meta.input.peek(Token![=]) {
        Ok(meta.value()?.parse::<syn::LitBool>()?.value)
    } else {
        Ok(true)
    }
}

/// Applies one core `ortho_config` field option or routes it to the
/// documentation parser.
///
/// Returns `false` only when no parser recognizes the key, allowing the caller
/// to preserve the established unknown-attribute behaviour.
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
            out.cli_default_as_absent = parse_cli_default_as_absent(meta)?;
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
/// provided, this function attempts to infer a default from clap's default
/// attributes. String defaults retain the field parser metadata so generated
/// code can reproduce clap's conversion at runtime.
///
/// Used internally by the derive macro to extract configuration metadata
/// from field-level attributes.
pub(crate) fn parse_field_attrs(field: &syn::Field) -> Result<FieldAttrs, syn::Error> {
    let mut out = FieldAttrs {
        is_subcommand: clap_field_is_subcommand(field)?,
        ..FieldAttrs::default()
    };
    if out.is_subcommand {
        reject_subcommand_ortho_config_attrs(field)?;
        return Ok(out);
    }
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
    }
    Ok(out)
}
