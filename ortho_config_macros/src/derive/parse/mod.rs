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
mod doc_attrs_common;
mod doc_types;
mod field_attrs;
mod input;
mod literals;
mod localization;
mod output_attrs;
mod renderer_attrs;
mod renderer_values;
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
    ExitClassAttr, HeadingOverrides, OutputAttrs, RenderWidthAttr, RendererFieldAttr,
    RendererRecordAttrs,
};
pub(crate) use field_attrs::parse_field_attrs;
pub(crate) use input::parse_input;
#[cfg(any(test, doctest))]
pub(crate) use literals::__doc_lit_str;
/// Re-exported for the `lit` tests, which reach the helpers through this
/// module's glob import. The production callers of `lit_char` now live in
/// [`struct_attrs`] and [`doc_attrs`], so without this the test-only path
/// would depend on an incidental private import surviving future splits.
#[cfg(any(test, doctest))]
pub(crate) use literals::lit_char;
pub(crate) use literals::lit_crate_path;
pub(crate) use output_attrs::parse_output_meta;
pub(crate) use renderer_attrs::parse_renderer_meta;
pub(crate) use serde_attrs::{
    SerdeRenameAll, serde_deserialized_field_key, serde_field_aliases,
    serde_field_has_custom_deserializer, serde_field_is_flattened, serde_field_rename,
    serde_field_skips_deserialization, serde_has_default, serde_rename_all, serde_rename_rules,
    serde_serialized_field_key,
};
pub(crate) use struct_attrs::{discard_unknown, parse_ortho_config, parse_struct_attrs};
pub(crate) use type_utils::{
    ClapDefaultValueShape, btree_map_inner, hash_map_inner, nested_schema_type, option_inner,
    vec_inner,
};

const _: fn(&Attribute, &mut Option<LitStr>) -> syn::Result<()> = clap_arg_id_from_attribute;
const _: fn(&syn::Field) -> syn::Result<bool> = clap_field_is_subcommand;
const _: fn(&syn::Variant) -> syn::Result<Option<LitStr>> = clap_variant_name;
const _: fn(&[Attribute]) -> syn::Result<Option<String>> = serde_field_rename;

/// Parsed struct options shared by derive validation and code generation.
///
/// Values are kept as syntax-level metadata until generation validates and
/// emits the corresponding runtime contract.
#[derive(Default, Clone)]
pub(crate) struct StructAttrs {
    /// Environment-variable prefix, normalized to end in `_` when non-empty.
    pub prefix: Option<String>,
    /// Optional file-discovery policy and naming overrides.
    pub discovery: Option<DiscoveryAttrs>,
    /// Whether generated loading calls the post-merge hook after composition.
    pub post_merge_hook: bool,
    /// Fluent catalogue root as a dotted path (D-5); falls back to docs app name.
    pub localization_base: Option<String>,
    /// Opts the struct into profile support (roadmap 9.1.1): the generated
    /// `--profile` flag, the `<PREFIX>PROFILE` selector, the profile merge
    /// layer, and `profiles.supported = true` in agent context.
    pub profiles: bool,
    /// Documentation metadata copied into generated API documentation.
    pub doc: DocStructAttrs,
    /// Overrides the generated crate path for dependency aliasing.
    ///
    /// When set via `#[ortho_config(crate = "my_alias")]`, generated code
    /// references types through `my_alias::` instead of `ortho_config::`.
    pub crate_path: Option<syn::Path>,
}

/// How a field participates in command-line parsing.
///
/// The three cases are mutually exclusive: a clap subcommand selector is not a
/// configuration value, whereas a skipped field is an ordinary configuration
/// value that the author deliberately removed from the parser. Call sites that
/// only care whether an option is rendered use [`Self::is_option`].
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliFieldMode {
    /// Rendered as a command-line option when the field's type supports one.
    #[default]
    Option,
    /// Excluded from the parser but still merged from other configuration
    /// layers.
    Skipped,
    /// A clap subcommand selector; never a configuration value.
    SubcommandSelector,
}

impl CliFieldMode {
    /// Whether the field is a clap subcommand selector.
    pub(crate) const fn is_subcommand(self) -> bool {
        matches!(self, Self::SubcommandSelector)
    }

    /// Whether the author excluded the field from command-line parsing.
    pub(crate) const fn is_skipped(self) -> bool {
        matches!(self, Self::Skipped)
    }

    /// Whether the field is rendered as an ordinary command-line option.
    pub(crate) const fn is_option(self) -> bool {
        matches!(self, Self::Option)
    }
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
/// - `nested` opts the field's type into required-value schema composition, so
///   absent leaves inside the nested object can be enumerated.
/// - `inferred_clap_default` stores the default inferred from clap's
///   `default_value`, `default_value_t`, or `default_values_t` when
///   `cli_default_as_absent` is
///   active and no explicit `#[ortho_config(default = ...)]` is provided.
#[derive(Default, Clone)]
pub(crate) struct FieldAttrs {
    /// Explicit long option spelling used by generated CLI metadata.
    pub cli_long: Option<String>,
    /// Explicit short option spelling used by generated CLI metadata.
    pub cli_short: Option<char>,
    /// User expression used when no configuration layer supplies the field.
    pub default: Option<Expr>,
    /// Clap default preserved when the CLI default must be treated as absent.
    pub inferred_clap_default: Option<ClapInferredDefault>,
    /// Collection policy applied as configuration layers are composed.
    pub merge_strategy: Option<MergeStrategy>,
    /// How the field participates in command-line parsing.
    pub cli: CliFieldMode,
    /// Omits a clap-provided default from the CLI merge layer.
    pub cli_default_as_absent: bool,
    /// Opts the field's type into required-value schema composition.
    pub nested: bool,
    /// Field documentation metadata copied into generated API documentation.
    pub doc: DocFieldAttrs,
}
