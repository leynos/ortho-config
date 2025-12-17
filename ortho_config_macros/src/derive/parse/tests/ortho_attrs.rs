//! Tests for `#[ortho_config(...)]` parsing behaviour.

use super::super::*;
use anyhow::{Result, anyhow, ensure};
use quote::quote;
use rstest::rstest;
use syn::{Attribute, DeriveInput, parse_quote};

/// Helper to assert that a `merge_strategy` attribute is correctly parsed.
struct MergeStrategyCase<'a> {
    strategy_name: &'a str,
    expected: MergeStrategy,
    struct_name: &'a str,
    field_name: &'a str,
    field_type: proc_macro2::TokenStream,
}

fn assert_merge_strategy(case: &MergeStrategyCase<'_>) -> Result<()> {
    let input: DeriveInput = syn::parse_str(&format!(
        r#"
        struct {struct_name} {{
            #[ortho_config(merge_strategy = "{strategy_name}")]
            {field_name}: {field_type},
        }}
        "#,
        struct_name = case.struct_name,
        strategy_name = case.strategy_name,
        field_name = case.field_name,
        field_type = &case.field_type,
    ))
    .map_err(|err| anyhow!("failed to parse input: {err}"))?;

    let (_, _, _, attrs_vec) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let attrs = attrs_vec
        .first()
        .ok_or_else(|| anyhow!("missing field attributes"))?;
    ensure!(
        attrs.merge_strategy == Some(case.expected),
        "{strategy} strategy not parsed",
        strategy = case.strategy_name,
    );
    Ok(())
}

#[test]
fn parses_struct_and_field_attributes() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(prefix = "CFG_")]
        struct Demo {
            #[ortho_config(cli_long = "opt", cli_short = 'o', default = 5)]
            field1: Option<u32>,
            #[ortho_config(merge_strategy = "append")]
            field2: Vec<String>,
        }
    };

    let (ident, fields, struct_attrs, field_attrs) =
        parse_input(&input).map_err(|err| anyhow!(err))?;

    ensure!(ident == "Demo", "expected Demo ident, got {ident}");
    ensure!(fields.len() == 2, "expected 2 fields, got {}", fields.len());
    ensure!(
        struct_attrs.prefix.as_deref() == Some("CFG_"),
        "expected CFG_ prefix"
    );
    ensure!(field_attrs.len() == 2, "expected 2 field attrs");
    ensure!(
        field_attrs
            .first()
            .and_then(|attrs| attrs.cli_long.as_deref())
            == Some("opt"),
        "expected first cli_long opt"
    );
    ensure!(
        field_attrs.first().and_then(|attrs| attrs.cli_short) == Some('o'),
        "expected first cli_short o"
    );
    ensure!(
        matches!(
            field_attrs.get(1).and_then(|attrs| attrs.merge_strategy),
            Some(MergeStrategy::Append)
        ),
        "expected second field append strategy"
    );
    Ok(())
}

/// Verify that a single-field `#[ortho_config(...)]` flag is correctly parsed.
fn assert_field_flag<F>(attribute: &str, check: F, error_msg: &str) -> Result<()>
where
    F: FnOnce(&FieldAttrs) -> bool,
{
    let input: DeriveInput = syn::parse_str(&format!(
        r"
        struct Demo {{
            #[ortho_config({attribute})]
            field: String,
        }}
        ",
    ))
    .map_err(|err| anyhow!("failed to parse input: {err}"))?;

    let (_, fields, _, field_attrs) = parse_input(&input).map_err(|err| anyhow!(err))?;
    ensure!(fields.len() == 1, "expected single field");
    let attrs = field_attrs
        .first()
        .ok_or_else(|| anyhow!("missing field attributes"))?;
    ensure!(check(attrs), "{error_msg}");
    Ok(())
}

#[test]
fn parses_skip_cli_flag() -> Result<()> {
    assert_field_flag(
        "skip_cli",
        |attrs| attrs.skip_cli,
        "skip_cli flag was not set",
    )
}

#[test]
fn parses_cli_default_as_absent_flag() -> Result<()> {
    assert_field_flag(
        "cli_default_as_absent",
        |attrs| attrs.cli_default_as_absent,
        "cli_default_as_absent flag was not set",
    )
}

#[test]
fn parses_cli_default_as_absent_false_disables_flag() -> Result<()> {
    assert_field_flag(
        "cli_default_as_absent = false",
        |attrs| !attrs.cli_default_as_absent,
        "cli_default_as_absent flag remained enabled",
    )
}

#[test]
fn parses_discovery_attributes() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(prefix = "CFG_", discovery(
            app_name = "demo",
            env_var = "DEMO_CONFIG",
            config_file_name = "demo.toml",
            dotfile_name = ".demo.toml",
            project_file_name = "demo-config.toml",
            config_cli_long = "config",
            config_cli_short = 'c',
            config_cli_visible = true,
        ))]
        struct Demo {
            value: u32,
        }
    };

    let (_, _, struct_attrs, _) = parse_input(&input).map_err(|err| anyhow!(err))?;
    let discovery = struct_attrs
        .discovery
        .ok_or_else(|| anyhow!("missing discovery attrs"))?;
    ensure!(
        discovery.app_name.as_deref() == Some("demo"),
        "app_name mismatch"
    );
    ensure!(
        discovery.env_var.as_deref() == Some("DEMO_CONFIG"),
        "env_var mismatch"
    );
    ensure!(
        discovery.config_file_name.as_deref() == Some("demo.toml"),
        "config_file_name mismatch"
    );
    ensure!(
        discovery.dotfile_name.as_deref() == Some(".demo.toml"),
        "dotfile mismatch"
    );
    ensure!(
        discovery.project_file_name.as_deref() == Some("demo-config.toml"),
        "project file mismatch"
    );
    ensure!(
        discovery.config_cli_long.as_deref() == Some("config"),
        "cli long mismatch"
    );
    ensure!(
        discovery.config_cli_short == Some('c'),
        "cli short mismatch"
    );
    ensure!(
        discovery.config_cli_visible == Some(true),
        "visibility mismatch"
    );
    Ok(())
}

#[rstest]
#[case::append(MergeStrategyCase {
    strategy_name: "append",
    expected: MergeStrategy::Append,
    struct_name: "AppendDemo",
    field_name: "values",
    field_type: quote!(Vec<String>),
})]
#[case::replace(MergeStrategyCase {
    strategy_name: "replace",
    expected: MergeStrategy::Replace,
    struct_name: "ReplaceDemo",
    field_name: "items",
    field_type: quote!(Vec<u32>),
})]
#[case::keyed(MergeStrategyCase {
    strategy_name: "keyed",
    expected: MergeStrategy::Keyed,
    struct_name: "KeyedDemo",
    field_name: "mapping",
    field_type: quote!(BTreeMap<String, String>),
})]
fn parses_merge_strategy(#[case] case: MergeStrategyCase<'static>) -> Result<()> {
    assert_merge_strategy(&case)
}

#[test]
fn parses_merge_strategy_invalid() -> Result<()> {
    let invalid: DeriveInput = parse_quote! {
        struct InvalidDemo {
            #[ortho_config(merge_strategy = "unknown")]
            values: Vec<String>,
        }
    };
    let err = parse_input(&invalid)
        .err()
        .ok_or_else(|| anyhow!("expected merge strategy error"))?;
    ensure!(
        err.to_string().contains("unknown merge_strategy"),
        "unexpected error message: {err}",
    );
    Ok(())
}

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
