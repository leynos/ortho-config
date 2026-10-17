//! Procedural macros for `ortho_config`.
//!
//! The current implementation of the [`OrthoConfig`] derive provides a basic
//! `load` method that layers configuration from a `config.toml` file,
//! environment variables, and now command-line arguments via `clap`. CLI flag
//! names are automatically generated from `snake_case` field names by replacing
//! underscores with hyphens. Generated long flags therefore never include
//! underscores; supply `cli_long` to specify them.

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

mod derive {
    //! Implementation modules for the `OrthoConfig` derive macro.

    pub(crate) mod build;
    pub(crate) mod crate_path;
    pub(crate) mod generate;
    pub(crate) mod load_impl;
    pub(crate) mod parse;
    pub(crate) mod policy_impl;
}
mod components;
pub(crate) use components::MacroComponents;
use components::{MacroComponentArgs, build_macro_components};
mod selected_subcommand_merge;
mod subcommand_docs;

use derive::generate::declarative::generate_declarative_impl;
use derive::generate::docs::{DocsArgs, generate_docs_impl};
use derive::generate::localization::{
    emit_identifier_artefact, emit_localization_impl, generate_localization_ids,
};
use derive::generate::ortho_impl::generate_trait_implementation;
use derive::parse::{parse_input, serde_rename_all};

/// Derive macro for the
/// [`OrthoConfig`](https://docs.rs/ortho_config/latest/ortho_config/trait.OrthoConfig.html)
/// trait.
///
/// # Errors
///
/// Returns a compile-time error if invoked on a struct that contains unnamed fields.
#[proc_macro_derive(OrthoConfig, attributes(ortho_config))]
pub fn derive_ortho_config(input_tokens: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input_tokens as DeriveInput);
    let (ident, fields, struct_attrs, field_attrs) = match parse_input(&derive_input) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let serde_rename_all = match serde_rename_all(&derive_input.attrs) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let krate = derive::crate_path::resolve(struct_attrs.crate_path.as_ref());

    let component_args = MacroComponentArgs {
        ident: &ident,
        fields: &fields,
        struct_attrs: &struct_attrs,
        field_attrs: &field_attrs,
        serde_rename_all,
        krate: &krate,
    };

    let components = match build_macro_components(&component_args) {
        Ok(v) => v,
        Err(e) => return e.to_compile_error().into(),
    };

    let core_tokens = generate_trait_implementation(&ident, &components, &krate);
    let declarative_impl = generate_declarative_impl(
        &ident,
        &components.collection_strategies,
        components.post_merge_hook,
        &krate,
    );
    let localization_ids =
        match generate_localization_ids(&struct_attrs, &ident, &fields, &field_attrs) {
            Ok(model) => model,
            Err(err) => return err.to_compile_error().into(),
        };
    let docs_impl = match generate_docs_impl(&DocsArgs {
        ident: &ident,
        fields: &fields,
        field_attrs: &field_attrs,
        struct_attrs: &struct_attrs,
        serde_rename_all,
        cli_fields: &components.cli_field_metadata,
        localization_ids: &localization_ids,
        krate: &krate,
    }) {
        Ok(tokens) => tokens,
        Err(err) => return err.to_compile_error().into(),
    };
    if let Err(err) = emit_identifier_artefact(&localization_ids, &ident) {
        return err.to_compile_error().into();
    }
    let localization_impl = emit_localization_impl(&localization_ids, &ident, &krate);
    let expanded = quote! {
        #core_tokens
        #declarative_impl
        #docs_impl
        #localization_impl
    };

    TokenStream::from(expanded)
}

/// Derive macro for the `ortho_config::SelectedSubcommandMerge` trait.
///
/// Apply this derive to a `clap::Subcommand` enum to generate a
/// `load_and_merge_selected` method that merges configuration defaults for the
/// selected variant.
///
/// Variants can opt into `ArgMatches`-aware merging (required for
/// `cli_default_as_absent` support) using `#[ortho_subcommand(with_matches)]`.
#[proc_macro_derive(SelectedSubcommandMerge, attributes(ortho_subcommand, ortho_config))]
pub fn derive_selected_subcommand_merge(input_tokens: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input_tokens as DeriveInput);
    match selected_subcommand_merge::derive_selected_subcommand_merge(derive_input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive macro for the `ortho_config::docs::OrthoConfigSubcommandDocs` trait.
///
/// Apply this derive to a `clap::Subcommand` enum whose variants each wrap a
/// single argument struct that implements `OrthoConfigDocs`.
#[proc_macro_derive(OrthoConfigSubcommandDocs, attributes(ortho_config))]
pub fn derive_subcommand_docs(input_tokens: TokenStream) -> TokenStream {
    let derive_input = parse_macro_input!(input_tokens as DeriveInput);
    match subcommand_docs::derive_subcommand_docs(derive_input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Metadata about a field used for `CliValueExtractor` generation.
#[derive(Clone)]
pub(crate) struct CliFieldInfo {
    /// The field name used by serde during serialization.
    ///
    /// This respects `#[serde(rename = "...")]` and `#[serde(rename_all = "...")]`
    /// so `CliValueExtractor` can reliably map values from `serde_json::to_value(self)`.
    pub serialized_key: String,
    /// The clap argument ID used with `ArgMatches::value_source()`.
    ///
    /// This is the `id = "..."` override from `#[arg(...)]`/`#[clap(...)]` when
    /// present, otherwise the field identifier (`snake_case`).
    pub arg_id: String,
    /// Whether the field's declared type is `bool` (or `Option<bool>`).
    ///
    /// Selects optional boolean-value parsing: absence remains `None`, a bare
    /// flag means `true`, and an explicit `=false` remains user input. Layer
    /// builders also use this classification to prune actual clap defaults
    /// without mistaking an explicit `false` for an absent value.
    pub is_bool: bool,
    /// Whether this field has the `cli_default_as_absent` attribute.
    pub is_default_as_absent: bool,
}

#[cfg(test)]
mod tests;
