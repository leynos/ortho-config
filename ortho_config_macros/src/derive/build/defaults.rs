//! Default struct helpers for the derive macro.
//!
//! These functions materialize the intermediate defaults struct that collects
//! per-field values before layered configuration is merged.

use quote::{format_ident, quote};

use crate::derive::parse::{
    ClapDefaultValue, ClapDefaultValueShape, ClapInferredDefault, FieldAttrs,
};

use super::cli::option_type_tokens;

fn require_named_field(field: &syn::Field) -> Result<&syn::Ident, proc_macro2::TokenStream> {
    field.ident.as_ref().ok_or_else(|| {
        syn::Error::new_spanned(field, "OrthoConfig defaults structs require named fields")
            .to_compile_error()
    })
}

pub(crate) fn build_default_struct_fields(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .zip(field_attrs.iter())
        .filter(|(_, attrs)| !attrs.is_subcommand)
        .map(|(f, _)| {
            let name = match require_named_field(f) {
                Ok(ident) => ident,
                Err(err) => return err,
            };
            let ty = option_type_tokens(&f.ty);
            quote! {
                #[serde(skip_serializing_if = "Option::is_none")]
                pub #name: #ty
            }
        })
        .collect()
}

/// Tokens that resolve defaults before constructing the defaults struct.
pub(crate) struct DefaultStructInit {
    pub resolutions: Vec<proc_macro2::TokenStream>,
    pub fields: Vec<proc_macro2::TokenStream>,
    pub cli_default_as_absent_fields: Vec<syn::LitStr>,
}

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
        .filter(|(_, attr)| !attr.is_subcommand)
        .for_each(|(f, attr)| {
            let name = match require_named_field(f) {
                Ok(ident) => ident,
                Err(err) => {
                    default_fields.push(err);
                    return;
                }
            };
            if attr.cli_default_as_absent {
                cli_default_as_absent_fields.push(syn::LitStr::new(&name.to_string(), name.span()));
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
