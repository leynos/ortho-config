//! Replay of clap parser settings onto the generated CLI struct fields.
//!
//! An inferred clap default is discovered from `#[arg(...)]` attributes such as
//! `default_value`. The generated argument must repeat the parser settings that
//! affect which values it accepts, otherwise a field that parses a value from
//! the configuration file could reject the same text on the command line.
//!
//! The settings are emitted as clap attribute fragments rather than values, so
//! this module stays independent of the surrounding field-generation logic.

use quote::quote;

use crate::derive::parse::{ClapInferredDefault, FieldAttrs};

/// Build the clap attribute fragments that replay an inferred default.
///
/// Returns an empty token stream when the field carries no inferred clap
/// default, so callers can splice the result into an `#[arg(...)]` attribute
/// unconditionally.
///
/// `is_bool` controls whether the default value itself is replayed. Only a
/// boolean flag replays `default_value`, so that clap records the floor value
/// under [`clap::parser::ValueSource::DefaultValue`] rather than as an explicit
/// `CommandLine` value. The generated layering guard reads exactly that
/// per-argument source, so an omitted boolean flag still defers to the
/// configuration file and the environment. Every other field skips this replay:
/// its inferred default reaches the merge as part of the defaults layer, and
/// the value is never attached to the generated argument.
pub(super) fn clap_replay_attributes(
    attrs: &FieldAttrs,
    is_bool: bool,
) -> proc_macro2::TokenStream {
    let Some(ClapInferredDefault::Value(default)) = attrs.inferred_clap_default.as_ref() else {
        return proc_macro2::TokenStream::new();
    };

    let bool_default = is_bool.then(|| {
        let value = &default.value;
        quote! { default_value = #value, }
    });
    let value_parser = default.value_parser.as_ref().map(|parser| {
        quote! { value_parser = #parser, }
    });
    let value_enum = default.value_enum.then(|| quote! { value_enum, });
    let value_delimiter = default.value_delimiter.as_ref().map(|delimiter| {
        quote! { value_delimiter = #delimiter, }
    });
    let ignore_case = default.ignore_case.as_ref().map(|ignore_case| {
        quote! { ignore_case = #ignore_case, }
    });

    quote! {
        #bool_default
        #value_parser
        #value_enum
        #value_delimiter
        #ignore_case
    }
}
