//! Generated `--profile` flag for opted-in structs (roadmap 9.1.1).
//!
//! When `#[ortho_config(profiles)]` is present, the CLI struct gains a hidden
//! global `--profile <NAME>` argument with a `<PREFIX>PROFILE` environment
//! fallback. The field is `#[serde(skip)]` so the selector never appears in
//! the serialized CLI layer (constraint 7). Opting in reserves the `profile`
//! root key, the `--profile` long flag, and the `<PREFIX>PROFILE` binding; a
//! user field that claims any of them is a compile-time error (decision D6).

use std::collections::HashSet;

use quote::quote_spanned;
use syn::Ident;

use crate::derive::build::env::compute_profile_env_var;
use crate::derive::parse::{
    FieldAttrs, SerdeRenameAll, StructAttrs, clap_field_env, serde_serialized_field_key,
};

use super::cli::validate_cli_long;

/// Borrowed inputs used to validate and generate the profile flag.
#[derive(Clone, Copy)]
pub(super) struct ProfileFlagBuildInputs<'a> {
    pub(super) fields: &'a [syn::Field],
    pub(super) field_attrs: &'a [FieldAttrs],
    pub(super) serde_rename_all: Option<SerdeRenameAll>,
    pub(super) used_longs: &'a HashSet<String>,
    pub(super) field_names: &'a HashSet<String>,
}

/// Build the generated `--profile` field for an opted-in struct.
///
/// Returns `None` for legacy structs so their CLI struct is byte-for-byte
/// unchanged. For opted-in structs, returns the field tokens or a
/// compile-time error when a user field collides on the `profile` key, the
/// `--profile` long flag, or the `<PREFIX>PROFILE` environment binding.
pub(super) fn build_profile_flag_field(
    struct_attrs: &StructAttrs,
    inputs: ProfileFlagBuildInputs<'_>,
) -> syn::Result<Option<proc_macro2::TokenStream>> {
    if !struct_attrs.profiles {
        return Ok(None);
    }
    let ProfileFlagBuildInputs {
        fields,
        field_attrs,
        serde_rename_all,
        used_longs,
        field_names,
    } = inputs;
    let name = Ident::new("profile", proc_macro2::Span::call_site());

    // Projection (a): the `profile` root key.
    if field_names.contains("profile") {
        return Err(syn::Error::new_spanned(
            &name,
            "generated profile field conflicts with user-defined field 'profile'",
        ));
    }
    validate_profile_serialized_key(fields, field_attrs, serde_rename_all)?;

    // Projection (b): the `--profile` long flag.
    validate_cli_long(&name, "profile")?;
    if used_longs.contains("profile") {
        return Err(syn::Error::new_spanned(
            &name,
            "duplicate `cli_long` value 'profile' conflicts with the generated profile flag",
        ));
    }

    // Projection (c): the `<PREFIX>PROFILE` environment binding.
    let selector_env = compute_profile_env_var(struct_attrs);
    validate_profile_environment_binding(fields, &selector_env)?;

    let env_lit = syn::LitStr::new(&selector_env, proc_macro2::Span::call_site());
    let span = name.span();
    Ok(Some(quote_spanned! { span =>
        #[arg(long = "profile", global = true, env = #env_lit, value_name = "NAME", hide = true)]
        #[serde(skip)]
        pub profile: Option<String>
    }))
}

fn validate_profile_serialized_key(
    fields: &[syn::Field],
    field_attrs: &[FieldAttrs],
    serde_rename_all: Option<SerdeRenameAll>,
) -> syn::Result<()> {
    for (field, attrs) in fields.iter().zip(field_attrs) {
        if attrs.is_subcommand {
            continue;
        }
        if serde_serialized_field_key(field, serde_rename_all)? == "profile" {
            return Err(syn::Error::new_spanned(
                field,
                "generated profile field conflicts with a field serialized as 'profile'",
            ));
        }
    }
    Ok(())
}

fn validate_profile_environment_binding(
    fields: &[syn::Field],
    selector_env: &str,
) -> syn::Result<()> {
    for field in fields {
        if clap_field_env(field)?.is_some_and(|env| env == selector_env) {
            return Err(syn::Error::new_spanned(
                field,
                format!(
                    "duplicate `env` value '{selector_env}' conflicts with the generated profile environment binding"
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Unit tests for profile flag token generation.

    use super::*;
    use crate::derive::build::build_cli_struct_fields;
    use crate::derive::parse::parse_input;
    use anyhow::{Result, anyhow, ensure};
    use rstest::rstest;

    fn build(input: &syn::DeriveInput) -> Result<syn::Result<Option<proc_macro2::TokenStream>>> {
        let (_, fields, struct_attrs, field_attrs) =
            parse_input(input).map_err(|err| anyhow!(err))?;
        let serde_rename_all = None;
        let cli_struct = build_cli_struct_fields(&fields, &field_attrs)?;
        Ok(build_profile_flag_field(
            &struct_attrs,
            ProfileFlagBuildInputs {
                fields: &fields,
                field_attrs: &field_attrs,
                serde_rename_all,
                used_longs: &cli_struct.used_longs,
                field_names: &cli_struct.field_names,
            },
        ))
    }

    #[test]
    fn legacy_struct_emits_no_profile_field() -> Result<()> {
        let input: syn::DeriveInput = syn::parse_quote! {
            struct Demo { retries: u32 }
        };
        let result = build(&input)?;
        ensure!(
            result?.is_none(),
            "legacy structs must gain no profile flag"
        );
        Ok(())
    }

    #[test]
    fn opted_in_struct_emits_hidden_global_flag() -> Result<()> {
        let input: syn::DeriveInput = syn::parse_quote! {
            #[ortho_config(prefix = "APP_", profiles)]
            struct Demo { retries: u32 }
        };
        let tokens = build(&input)??.ok_or_else(|| anyhow!("expected a profile field"))?;
        let rendered = tokens.to_string();
        ensure!(
            rendered.contains("long = \"profile\""),
            "expected the --profile long flag: {rendered}"
        );
        ensure!(
            rendered.contains("global = true"),
            "expected a global flag: {rendered}"
        );
        ensure!(
            rendered.contains("env = \"APP_PROFILE\""),
            "expected the APP_PROFILE environment fallback: {rendered}"
        );
        ensure!(
            rendered.contains("hide = true"),
            "expected a hidden flag: {rendered}"
        );
        ensure!(
            rendered.contains("# [serde (skip)]"),
            "expected the selector to be excluded from the CLI layer: {rendered}"
        );
        Ok(())
    }

    #[rstest]
    #[case::field_name(
        syn::parse_quote! {
            #[ortho_config(profiles)]
            struct Demo { profile: Option<String> }
        },
        "conflicts with user-defined field 'profile'"
    )]
    #[case::serialized_key(
        syn::parse_quote! {
            #[ortho_config(profiles)]
            struct Demo {
                #[serde(rename = "profile")]
                selected_profile: Option<String>,
            }
        },
        "conflicts with a field serialized as 'profile'"
    )]
    #[case::long_flag(
        syn::parse_quote! {
            #[ortho_config(profiles)]
            struct Demo {
                #[ortho_config(cli_long = "profile")]
                thing: Option<String>,
            }
        },
        "duplicate `cli_long` value 'profile'"
    )]
    #[case::environment_binding(
        syn::parse_quote! {
            #[ortho_config(prefix = "APP_", profiles)]
            struct Demo {
                #[arg(env = "APP_PROFILE")]
                thing: Option<String>,
            }
        },
        "duplicate `env` value 'APP_PROFILE'"
    )]
    fn profile_collision_is_rejected(
        #[case] input: syn::DeriveInput,
        #[case] expected_error: &str,
    ) -> anyhow::Result<()> {
        let err = build(&input)?.expect_err("collision must error");
        ensure!(
            err.to_string().contains(expected_error),
            "unexpected error: {err}"
        );
        Ok(())
    }
}
