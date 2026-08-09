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

use syn::{Attribute, Expr, LitStr};

mod behaviour_attrs;
mod clap_attrs;
mod clap_defaults;
mod discovery_attrs;
mod doc_attrs;
mod doc_types;
mod field_attrs;
mod input;
mod literals;
mod localization;
mod serde_attrs;
mod struct_attrs;
#[cfg(test)]
mod tests;
mod type_utils;

pub(crate) use behaviour_attrs::parse_behaviour_meta;
pub(crate) use clap_attrs::{
    clap_arg_id_from_attribute, clap_arg_id_or_field_name, clap_field_env, clap_field_is_flattened,
    clap_field_is_subcommand, clap_variant_name, reject_subcommand_ortho_config_attrs,
};
pub(crate) use clap_defaults::{ClapDefaultValue, ClapInferredDefault, clap_default_value};
pub(crate) use discovery_attrs::{DiscoveryAttrs, MergeStrategy, parse_prefix};
use doc_attrs::apply_field_doc_attr;
pub(crate) use doc_types::{
    BehaviourAttrs, DocExampleAttr, DocFieldAttrs, DocLinkAttr, DocNoteAttr, DocStructAttrs,
    HeadingOverrides,
};
pub(crate) use field_attrs::parse_field_attrs;
pub(crate) use input::parse_input;
#[cfg(any(test, doctest))]
pub(crate) use literals::__doc_lit_str;
pub(crate) use literals::lit_crate_path;
pub(crate) use serde_attrs::{
    SerdeRenameAll, serde_field_rename, serde_has_default, serde_rename_all,
    serde_serialized_field_key,
};
pub(crate) use struct_attrs::{discard_unknown, parse_ortho_config, parse_struct_attrs};
pub(crate) use type_utils::{
    ClapDefaultValueShape, btree_map_inner, hash_map_inner, option_inner, vec_inner,
};

const _: fn(&Attribute, &mut Option<LitStr>) -> syn::Result<()> = clap_arg_id_from_attribute;
const _: fn(&syn::Field) -> syn::Result<bool> = clap_field_is_subcommand;
const _: fn(&syn::Variant) -> syn::Result<Option<LitStr>> = clap_variant_name;
const _: fn(&[Attribute]) -> syn::Result<Option<String>> = serde_field_rename;

#[derive(Default, Clone)]
pub(crate) struct StructAttrs {
    pub prefix: Option<String>,
    pub discovery: Option<DiscoveryAttrs>,
    pub post_merge_hook: bool,
    /// Fluent catalogue root as a dotted path (D-5); falls back to docs app name.
    pub localization_base: Option<String>,
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
///   `default_value`, `default_value_t`, or `default_values_t` when
///   `cli_default_as_absent` is
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
