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

use syn::{Attribute, Expr, LitStr, Token};

mod clap_attrs;
mod doc_attrs;
mod doc_types;
mod input;
mod literals;
mod serde_attrs;
mod struct_attrs;
#[cfg(test)]
mod tests;
mod type_utils;

pub(crate) use clap_attrs::{
    ClapInferredDefault, clap_arg_id, clap_arg_id_from_attribute, clap_default_value,
    clap_field_env, clap_field_is_subcommand, clap_variant_name,
    reject_subcommand_ortho_config_attrs,
};
use doc_attrs::apply_field_doc_attr;
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
pub(crate) use struct_attrs::{discard_unknown, parse_ortho_config, parse_struct_attrs};
pub(crate) use type_utils::{btree_map_inner, hash_map_inner, option_inner, vec_inner};

const _: fn(&Attribute, &mut Option<LitStr>) -> syn::Result<()> = clap_arg_id_from_attribute;
const _: fn(&syn::Field) -> syn::Result<bool> = clap_field_is_subcommand;
const _: fn(&syn::Variant) -> syn::Result<Option<LitStr>> = clap_variant_name;
const _: fn(&[Attribute]) -> syn::Result<Option<String>> = serde_field_rename;

#[derive(Default, Clone)]
pub(crate) struct StructAttrs {
    pub prefix: Option<String>,
    pub discovery: Option<DiscoveryAttrs>,
    pub post_merge_hook: bool,
    /// Opts the struct into profile support (roadmap 9.1.1): the generated
    /// `--profile` flag, the `<PREFIX>PROFILE` selector, the profile merge
    /// layer, and `profiles.supported = true` in agent context.
    pub profiles: bool,
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
    pub is_subcommand: bool,
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
fn parse_cli_default_as_absent(meta: &syn::meta::ParseNestedMeta) -> Result<bool, syn::Error> {
    if meta.input.peek(Token![=]) {
        Ok(meta.value()?.parse::<syn::LitBool>()?.value)
    } else {
        Ok(true)
    }
}

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
/// provided, this function attempts to infer a default from clap's
/// `default_value_t` or `default_values_t`. Inference from the untyped
/// `default_value` is rejected with a compile-time error.
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
