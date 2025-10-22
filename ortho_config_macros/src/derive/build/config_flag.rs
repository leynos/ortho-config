//! Config flag helpers used by the derive macro.
//!
//! These helpers construct the optional `--config-path` CLI argument and ensure
//! that it does not collide with user-defined fields or custom flag metadata.

use std::collections::HashSet;

use quote::{quote, quote_spanned};
use syn::Ident;

use crate::derive::parse::StructAttrs;

use super::cli::{validate_cli_long, validate_user_cli_short};

pub(crate) fn build_config_flag_field(
    struct_attrs: &StructAttrs,
    used_shorts: &HashSet<char>,
    used_longs: &HashSet<String>,
    existing_fields: &HashSet<String>,
) -> syn::Result<proc_macro2::TokenStream> {
    let name = Ident::new("config_path", proc_macro2::Span::call_site());
    if existing_fields.contains("config_path") {
        return Err(syn::Error::new_spanned(
            &name,
            "generated config flag field conflicts with user-defined field 'config_path'",
        ));
    }
    let discovery = struct_attrs.discovery.as_ref();
    let long = discovery
        .and_then(|attrs| attrs.config_cli_long.clone())
        .unwrap_or_else(|| String::from("config-path"));
    validate_cli_long(&name, &long)?;
    if used_longs.contains(&long) {
        return Err(syn::Error::new_spanned(
            &name,
            format!("duplicate `cli_long` value '{long}' conflicts with the generated config flag",),
        ));
    }
    let long_lit = syn::LitStr::new(&long, proc_macro2::Span::call_site());
    let mut arg_meta: Vec<proc_macro2::TokenStream> = vec![quote! { long = #long_lit }];
    if let Some(short) = discovery.and_then(|attrs| attrs.config_cli_short) {
        let claimed = validate_user_cli_short(&name, short, used_shorts)?;
        let short_lit = syn::LitChar::new(claimed, proc_macro2::Span::call_site());
        arg_meta.push(quote! { short = #short_lit });
    }
    let visible = discovery
        .and_then(|attrs| attrs.config_cli_visible)
        .unwrap_or(false);
    if !visible {
        arg_meta.push(quote! { hide = true });
    }
    arg_meta.push(quote! { value_name = "PATH" });
    if visible {
        arg_meta.push(quote! { help = "Path to the configuration file" });
    }
    let span = name.span();
    let serde_attr = quote_spanned! { span => #[serde(skip_serializing_if = "Option::is_none")] };
    Ok(quote_spanned! { span =>
        #[arg( #( #arg_meta ),* )]
        #serde_attr
        pub config_path: Option<std::path::PathBuf>
    })
}

#[cfg(test)]
mod tests {
    #![allow(
        unfulfilled_lint_expectations,
        reason = "clippy::expect_used is denied globally; tests may not hit those branches"
    )]
    #![expect(
        clippy::expect_used,
        reason = "tests panic to surface configuration mistakes"
    )]
    use super::*;
    use crate::derive::build::build_cli_struct_fields;
    use crate::derive::parse::{DiscoveryAttrs, StructAttrs};
    use rstest::rstest;

    #[rstest]
    #[case::long(
        syn::parse_quote! {
            struct Demo {
                #[ortho_config(cli_long = "config")]
                value: u32,
            }
        },
        DiscoveryAttrs {
            config_cli_long: Some(String::from("config")),
            ..DiscoveryAttrs::default()
        },
        "duplicate `cli_long` value",
    )]
    #[case::short(
        syn::parse_quote! {
            struct Demo {
                value: u32,
            }
        },
        DiscoveryAttrs {
            config_cli_short: Some('v'),
            ..DiscoveryAttrs::default()
        },
        "duplicate `cli_short` value",
    )]
    fn config_flag_rejects_duplicate_from_fields(
        #[case] input: syn::DeriveInput,
        #[case] discovery_attrs: DiscoveryAttrs,
        #[case] expected_error: &str,
    ) {
        let (_, fields, mut struct_attrs, field_attrs) =
            crate::derive::parse::parse_input(&input).expect("parse_input");
        let cli = build_cli_struct_fields(&fields, &field_attrs).expect("build cli fields");
        struct_attrs.discovery = Some(discovery_attrs);
        let err = build_config_flag_field(
            &struct_attrs,
            &cli.used_shorts,
            &cli.used_longs,
            &cli.field_names,
        )
        .expect_err("should fail");
        assert!(err.to_string().contains(expected_error));
    }

    #[rstest]
    #[case(syn::parse_quote! {
        struct Demo {
            #[ortho_config(cli_long = "alpha")]
            field1: u32,
            #[ortho_config(cli_long = "alpha")]
            field2: u32,
        }
    })]
    #[case(syn::parse_quote! {
        struct Demo {
            field_one: u32,
            #[ortho_config(cli_long = "field-one")]
            field_two: u32,
        }
    })]
    fn rejects_duplicate_long_flags_scenarios(#[case] input: syn::DeriveInput) {
        let (_, fields, _, field_attrs) =
            crate::derive::parse::parse_input(&input).expect("parse_input");
        let err = build_cli_struct_fields(&fields, &field_attrs).expect_err("should fail");
        assert!(err.to_string().contains("duplicate `cli_long` value"));
    }

    #[test]
    fn bool_fields_do_not_emit_skip_serializing_if() {
        #[derive(serde::Serialize)]
        struct __Cli {
            excited: Option<bool>,
        }

        let input: syn::DeriveInput = syn::parse_quote! {
            struct Demo {
                excited: bool,
            }
        };
        let (_, fields, _, field_attrs) =
            crate::derive::parse::parse_input(&input).expect("parse_input");
        let tokens = build_cli_struct_fields(&fields, &field_attrs).expect("build cli fields");
        let field_ts = tokens
            .fields
            .first()
            .expect("generated field tokens")
            .to_string();
        assert!(
            field_ts.contains("ArgAction :: SetTrue"),
            "boolean CLI fields should use ArgAction::SetTrue",
        );
        assert!(
            !field_ts.contains("skip_serializing_if"),
            "boolean CLI fields should not emit skip_serializing_if",
        );

        let cli = __Cli { excited: None };
        let figment = ortho_config::figment::Figment::from(
            ortho_config::figment::providers::Serialized::defaults(&cli),
        );
        assert!(
            figment.extract_inner::<bool>("excited").is_err(),
            "Absent boolean flags should not appear in Figment",
        );
    }

    #[test]
    fn config_flag_field_name_conflict_errors() {
        let used_shorts = HashSet::new();
        let used_longs = HashSet::new();
        let mut existing = HashSet::new();
        existing.insert(String::from("config_path"));
        let err = build_config_flag_field(
            &StructAttrs::default(),
            &used_shorts,
            &used_longs,
            &existing,
        )
        .expect_err("should fail on name conflict");
        assert!(
            err.to_string()
                .contains("generated config flag field conflicts with user-defined field")
        );
    }
}
