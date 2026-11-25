//! Tests for unknown `#[ortho_config(...)]` keys and prefix normalization.

use super::super::*;
use super::ortho_attrs::{assert_crate_path, assert_post_merge_hook, assert_profiles};
use anyhow::{Result, anyhow, ensure};
use rstest::rstest;
use syn::{DeriveInput, parse_quote};

#[rstest]
#[case::unknown_key(
    parse_quote! {
        #[ortho_config(prefix = "CFG_", unknown = "ignored")]
        struct Demo {
            #[ortho_config(bad_key)]
            field1: String,
        }
    },
    None
)]
#[case::unknown_key_with_value(
    parse_quote! {
        #[ortho_config(prefix = "CFG_", unexpected = 42)]
        struct Demo {
            #[ortho_config(cli_long = "f1", extra = true)]
            field1: String,
        }
    },
    Some("f1")
)]
#[case::multiple_unknown_keys(
    parse_quote! {
        #[ortho_config(foo, bar, prefix = "CFG_")]
        struct Demo {
            #[ortho_config(baz, qux, cli_long = "f1")]
            field1: String,
        }
    },
    Some("f1")
)]
#[case::mixed_order(
    parse_quote! {
        #[ortho_config(alpha, prefix = "CFG_", omega)]
        struct Demo {
            #[ortho_config(beta, cli_long = "f1", gamma)]
            field1: String,
        }
    },
    Some("f1")
)]
fn test_unknown_keys_handling(
    #[case] input: DeriveInput,
    #[case] cli_long: Option<&str>,
) -> Result<()> {
    let (_ident, fields, struct_attrs, field_attrs) =
        parse_input(&input).map_err(|err| anyhow!(err))?;

    ensure!(fields.len() == 1, "expected single field");
    ensure!(
        struct_attrs.prefix.as_deref() == Some("CFG_"),
        "expected CFG_ prefix"
    );
    let parsed = field_attrs
        .first()
        .and_then(|attrs| attrs.cli_long.as_deref());
    ensure!(
        parsed == cli_long,
        "cli_long mismatch: {parsed:?} != {cli_long:?}"
    );
    Ok(())
}

#[rstest]
#[case::missing_suffix("APP", "APP_")]
#[case::with_suffix("APP_", "APP_")]
#[case::empty("", "")]
fn struct_prefix_normalises_trailing_underscore(
    #[case] raw: &str,
    #[case] expected: &str,
) -> Result<()> {
    let lit = syn::LitStr::new(raw, proc_macro2::Span::call_site());
    let attr: Attribute = syn::parse_quote!(#[ortho_config(prefix = #lit)]);
    let attrs = parse_struct_attrs(&[attr]).map_err(|err| anyhow!(err))?;

    ensure!(
        attrs.prefix.as_deref() == Some(expected),
        "prefix normalisation mismatch"
    );
    Ok(())
}

#[rstest]
#[case::short_form(
    parse_quote! {
        #[ortho_config(post_merge_hook)]
        struct Config { field: String }
    },
    true,
    "post_merge_hook should be true when using short form"
)]
#[case::explicit_true(
    parse_quote! {
        #[ortho_config(post_merge_hook = true)]
        struct Config { field: String }
    },
    true,
    "post_merge_hook should be true when explicitly set to true"
)]
#[case::explicit_false(
    parse_quote! {
        #[ortho_config(post_merge_hook = false)]
        struct Config { field: String }
    },
    false,
    "post_merge_hook should be false when explicitly set to false"
)]
fn parses_post_merge_hook(
    #[case] input: DeriveInput,
    #[case] expected: bool,
    #[case] error_msg: &str,
) -> Result<()> {
    assert_post_merge_hook(&input, expected, error_msg)
}

#[test]
fn post_merge_hook_defaults_to_false() -> Result<()> {
    assert_post_merge_hook(
        &parse_quote! {
            struct Config { field: String }
        },
        false,
        "post_merge_hook should default to false when not specified",
    )
}

#[rstest]
#[case(
    parse_quote! {
        #[ortho_config(profiles)]
        struct Config { field: String }
    },
    true,
    "a bare profiles key should enable profile support"
)]
#[case(
    parse_quote! {
        #[ortho_config(profiles = false)]
        struct Config { field: String }
    },
    false,
    "profiles should be false when explicitly set to false"
)]
fn parses_profiles(
    #[case] input: DeriveInput,
    #[case] expected: bool,
    #[case] error_msg: &str,
) -> Result<()> {
    assert_profiles(&input, expected, error_msg)
}

#[test]
fn profiles_defaults_to_false() -> Result<()> {
    assert_profiles(
        &parse_quote! {
            struct Config { field: String }
        },
        false,
        "profiles should default to false when not specified",
    )
}

#[test]
fn parses_crate_path_simple() -> Result<()> {
    assert_crate_path(
        &parse_quote! {
            #[ortho_config(crate = "my_config")]
            struct Config { field: String }
        },
        "my_config",
        None,
        "simple crate path should parse as my_config",
    )
}

#[test]
fn parses_crate_path_nested() -> Result<()> {
    assert_crate_path(
        &parse_quote! {
            #[ortho_config(crate = "my_ns::ortho_config")]
            struct Config { field: String }
        },
        "my_ns :: ortho_config",
        None,
        "nested crate path should parse as my_ns :: ortho_config",
    )
}

#[test]
fn crate_path_defaults_to_none() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        struct Config { field: String }
    };
    let (_, _, struct_attrs, _) = parse_input(&input).map_err(|err| anyhow!(err))?;
    ensure!(
        struct_attrs.crate_path.is_none(),
        "expected crate_path to be None when omitted"
    );
    Ok(())
}

#[test]
fn parses_crate_path_with_prefix() -> Result<()> {
    assert_crate_path(
        &parse_quote! {
            #[ortho_config(prefix = "APP_", crate = "my_config")]
            struct Config { field: String }
        },
        "my_config",
        Some("APP_"),
        "crate path should parse alongside prefix",
    )
}

/// Unknown values that carry a top-level comma must still parse, and the
/// separator after them must be left unread.
///
/// `skip_to_item_end` stops at an item separator, but three constructs present
/// a comma that is not a separator: a turbofish's argument list, a closure's
/// parameter list, and that list behind `move`/`async`/`static`. The previous
/// `syn::Expr` parse accepted every one, so splitting them here would break
/// derives that compiled before. Each case also names a trailing key, which
/// fails if the scan consumes the separator instead of leaving it.
#[rstest]
#[case::turbofish_value(
    parse_quote! { #[ortho_config(prefix = "CFG_", future = factory::<Alpha, Beta>(), crate = "my_config")] },
)]
#[case::nested_turbofish_value(
    parse_quote! { #[ortho_config(prefix = "CFG_", future = outer::<Inner<Alpha, Beta>, Gamma>(), crate = "my_config")] },
)]
#[case::closure_two_parameters(
    parse_quote! { #[ortho_config(prefix = "CFG_", future = |a, b| a, crate = "my_config")] },
)]
#[case::closure_zero_parameters(
    parse_quote! { #[ortho_config(prefix = "CFG_", future = || 1, crate = "my_config")] },
)]
#[case::move_closure(
    parse_quote! { #[ortho_config(prefix = "CFG_", future = move |a, b| a, crate = "my_config")] },
)]
#[case::async_move_closure(
    parse_quote! { #[ortho_config(prefix = "CFG_", future = async move |a, b| a, crate = "my_config")] },
)]
#[case::bitwise_or_is_not_a_closure(
    parse_quote! { #[ortho_config(prefix = "CFG_", future = a | b, crate = "my_config")] },
)]
fn unknown_value_with_top_level_comma_leaves_the_next_key_readable(
    #[case] attribute: syn::Attribute,
) -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #attribute
        struct Demo { field: String }
    };
    let (_, _, struct_attrs, _) = parse_input(&input).map_err(|err| anyhow!(err))?;
    ensure!(
        struct_attrs.prefix.as_deref() == Some("CFG_"),
        "the known key before the unknown value must still be read"
    );
    ensure!(
        struct_attrs
            .crate_path
            .as_ref()
            .is_some_and(|path| path.is_ident("my_config")),
        "the key after the unknown value must still be read"
    );
    Ok(())
}
