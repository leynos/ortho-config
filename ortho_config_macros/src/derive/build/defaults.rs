//! Default struct helpers for the derive macro.
//!
//! These functions materialize the intermediate defaults struct that collects
//! per-field values before layered configuration is merged.

use quote::{format_ident, quote};

use crate::derive::parse::{
    ClapDefaultValue, ClapDefaultValueShape, ClapInferredDefault, FieldAttrs,
};

use super::cli::option_type_tokens;

/// Returns a field identifier or emits a compile error for unsupported tuple
/// input that escaped the earlier derive validation.
fn require_named_field(field: &syn::Field) -> Result<&syn::Ident, proc_macro2::TokenStream> {
    field.ident.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(field, "OrthoConfig defaults structs require named fields")
            .to_compile_error()
    })
}

/// Builds optional fields for the synthetic defaults struct, preserving the
/// configuration struct's order and excluding subcommand selectors.
pub(crate) fn build_default_struct_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .zip(field_attrs.iter())
        .filter(|(_, attrs)| !attrs.cli.is_subcommand())
        .map(|(f, attrs)| {
            let name = match require_named_field(f) {
                Ok(ident) => ident,
                Err(err) => return err,
            };
            let ty = option_type_tokens(&f.ty);
            // The defaults struct serializes to the lowest-precedence layer, so
            // it must use the configuration key rather than the Rust field
            // name; otherwise a renamed default is never read by the final type.
            let key_lit = syn::LitStr::new(&attrs.serialized_key, name.span());
            quote! {
                #[serde(rename = #key_lit, skip_serializing_if = "Option::is_none")]
                pub #name: #ty
            }
        })
        .collect()
}

/// Tokens that resolve defaults before constructing the defaults struct.
pub(crate) struct DefaultStructInit {
    /// Expressions evaluated before construction to report invalid Clap
    /// defaults through the generated error collection.
    pub resolutions: Vec<proc_macro2::TokenStream>,
    /// Initializer entries in source-field order, with explicit defaults taking
    /// precedence over inferred Clap defaults.
    pub fields: Vec<proc_macro2::TokenStream>,
    /// `(arg_id, layer_key)` pairs for fields whose parser defaults must be
    /// treated as absent when the user did not explicitly supply a CLI value.
    ///
    /// The argument ID answers `ArgMatches::value_source`; the layer key names
    /// the member to remove from the serialized CLI object. They differ for a
    /// field with `#[serde(rename)]`.
    pub cli_default_as_absent_fields: Vec<(syn::LitStr, syn::LitStr)>,
}

/// Resolves field defaults in the same order as runtime configuration loading.
///
/// Explicit `OrthoConfig` defaults win over inferred Clap defaults; fields with
/// neither remain absent for lower-precedence layers to supply.
pub(crate) fn build_default_struct_init(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
    krate: &proc_macro2::TokenStream,
) -> DefaultStructInit {
    let mut resolutions = Vec::new();
    let mut default_fields = Vec::new();
    let mut cli_default_as_absent_fields = Vec::new();
    fields
        .iter()
        .zip(field_attrs.iter())
        .filter(|(_, attr)| !attr.cli.is_subcommand())
        .for_each(|(f, attr)| {
            let name = match require_named_field(f) {
                Ok(ident) => ident,
                Err(err) => {
                    default_fields.push(err);
                    return;
                }
            };
            if attr.cli_default_as_absent {
                cli_default_as_absent_fields.push((
                    syn::LitStr::new(&name.to_string(), name.span()),
                    syn::LitStr::new(&attr.serialized_key, name.span()),
                ));
            }
            if let Some(expr) = &attr.default {
                default_fields.push(quote! { #name: Some(#expr) });
                return;
            }
            match attr.inferred_clap_default.as_ref() {
                Some(ClapInferredDefault::Value(default)) => {
                    let resolution_ident = format_ident!("__ortho_config_default_{name}");
                    let value_expr = clap_value_default_expr(default);
                    let field_key = name.to_string();
                    resolutions.push(quote! {
                        let #resolution_ident = match #value_expr {
                            Ok(value) => Some(value),
                            Err(()) => {
                                errors.push(#krate::OrthoError::default_value_conversion_arc(
                                    #field_key,
                                ));
                                None
                            }
                        };
                    });
                    default_fields.push(quote! { #name: #resolution_ident });
                }
                Some(ClapInferredDefault::ValueT(expr)) => {
                    default_fields.push(quote! { #name: Some(#expr) });
                }
                Some(ClapInferredDefault::ValuesT(expr)) => {
                    default_fields.push(quote! {
                        #name: Some(
                            ::std::iter::IntoIterator::into_iter(#expr)
                                .collect::<::std::vec::Vec<_>>()
                        )
                    });
                }
                None => default_fields.push(quote! { #name: None }),
            }
        });
    DefaultStructInit {
        resolutions,
        fields: default_fields,
        cli_default_as_absent_fields,
    }
}

/// Emits code that asks Clap's configured parser to materialize one inferred
/// default, preserving its value parser, delimiter, and case settings.
fn clap_value_default_expr(default: &ClapDefaultValue) -> proc_macro2::TokenStream {
    let value = &default.value;
    let leaf_type = &default.leaf_type;
    let parser = default.value_parser.as_ref().map_or_else(
        || {
            if default.value_enum {
                quote! { ::clap::builder::EnumValueParser::<#leaf_type>::new() }
            } else {
                quote! { ::clap::value_parser!(#leaf_type) }
            }
        },
        |value_parser| quote! { #value_parser },
    );
    let value_delimiter = default
        .value_delimiter
        .as_ref()
        .map(|delimiter| quote! { .value_delimiter(#delimiter) });
    let ignore_case = default
        .ignore_case
        .as_ref()
        .map(|ignore_case| quote! { .ignore_case(#ignore_case) });
    let extraction = match default.shape {
        ClapDefaultValueShape::Scalar | ClapDefaultValueShape::Option => quote! {
            matches
                .try_remove_one::<#leaf_type>("value")
                .map_err(|_| ())?
                .ok_or(())
        },
        ClapDefaultValueShape::Vec => quote! {
            matches
                .try_remove_many::<#leaf_type>("value")
                .map_err(|_| ())?
                .map(|values| values.collect::<::std::vec::Vec<_>>())
                .ok_or(())
        },
    };

    quote! {
        (|| {
            let mut command = ::clap::Command::new("ortho-config-default")
                .arg(
                    ::clap::Arg::new("value")
                        .action(::clap::ArgAction::Append)
                        .default_value(#value)
                        .value_parser(#parser)
                        #value_delimiter
                        #ignore_case,
                );
            let mut matches = command
                .try_get_matches_from_mut(["ortho-config-default"])
                .map_err(|_| ())?;
            #extraction
        })()
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the generated defaults struct and its pruning metadata.

    use super::{build_default_struct_fields, build_default_struct_init};
    use crate::derive::parse::parse_input;
    use anyhow::{Result, anyhow, ensure};
    use quote::quote;

    /// A renamed field's defaults entry serializes under the configuration key.
    ///
    /// The defaults layer is deserialized by the final configuration type, so
    /// keeping the Rust identifier would make a declared default invisible.
    #[test]
    fn defaults_fields_serialize_under_the_config_key() -> Result<()> {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[serde(rename_all = "kebab-case")]
            struct Demo {
                #[ortho_config(default = 3)]
                retry_count: u32,
            }
        };
        let (_, fields, _, attrs) = parse_input(&input).map_err(|err| anyhow!(err))?;
        let rendered = build_default_struct_fields(&fields, &attrs)
            .first()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| anyhow!("expected one generated defaults field"))?;
        ensure!(
            rendered.contains("rename = \"retry-count\""),
            "expected the defaults field to serialize under the configuration key: {rendered}"
        );
        Ok(())
    }

    /// The pruning metadata pairs the clap argument ID with the layer key.
    ///
    /// Pruning asks clap about the argument and removes the serialized member,
    /// which differ after a rename.
    #[test]
    fn default_as_absent_metadata_keeps_both_keys() -> Result<()> {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct Demo {
                #[serde(rename = "retry-count")]
                #[arg(long, default_value_t = 3)]
                #[ortho_config(cli_default_as_absent)]
                retry_count: u32,
            }
        };
        let (_, fields, _, attrs) = parse_input(&input).map_err(|err| anyhow!(err))?;
        let init = build_default_struct_init(&fields, &attrs, &quote! { ortho_config });
        let (arg_id, layer_key) = init
            .cli_default_as_absent_fields
            .first()
            .ok_or_else(|| anyhow!("expected pruning metadata for the field"))?;
        ensure!(
            arg_id.value() == "retry_count" && layer_key.value() == "retry-count",
            "expected (retry_count, retry-count), got ({}, {})",
            arg_id.value(),
            layer_key.value()
        );
        Ok(())
    }
}
