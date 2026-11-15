//! Unit tests for the required-value schema generator.
//!
//! Each case parses a struct, emits its schema implementation, and asserts on
//! the normalized token text. Assertions target the decisions the generator
//! makes — crate path, generic bounds, key naming, requiredness, and hints —
//! rather than the whole expansion, so unrelated rendering changes do not
//! break them.

use anyhow::{Result, anyhow, ensure};
use rstest::rstest;
use syn::DeriveInput;

use crate::derive::build::CliFieldMetadata;
use crate::derive::generate::required_schema::RequiredSchemaSource;

#[path = "tests_deserializers.rs"]
mod deserializers;

/// Normalize token spacing so assertions can use readable fragments.
fn normalize(tokens: &proc_macro2::TokenStream) -> String {
    tokens.to_string().replace(" :: ", "::").replace(' ', "")
}

/// Emit the schema implementation for `input`, with optional CLI metadata.
fn emit(input: &DeriveInput, metadata: &[CliFieldMetadata]) -> Result<String> {
    let (ident, fields, struct_attrs, field_attrs) =
        crate::derive::parse::parse_input(input).map_err(|error| anyhow!("{error}"))?;
    let krate = crate::derive::crate_path::resolve(struct_attrs.crate_path.as_ref());
    let tokens = RequiredSchemaSource {
        ident: &ident,
        generics: &input.generics,
        fields: &fields,
        field_attrs: &field_attrs,
        struct_attrs: &struct_attrs,
        container_attrs: &input.attrs,
        cli_metadata: metadata,
        krate: &krate,
    }
    .generate()
    .map_err(|error| anyhow!("{error}"))?;
    Ok(normalize(&tokens))
}

/// Emit with no CLI metadata, matching the schema-only derive.
fn emit_plain(input: &DeriveInput) -> Result<String> {
    emit(input, &[])
}

/// A CLI hint for `field_name` with the same long spelling.
fn hint(field_name: &str, is_bool: bool) -> CliFieldMetadata {
    CliFieldMetadata {
        field_name: String::from(field_name),
        long: String::from(field_name),
        short: 'p',
        is_bool,
    }
}

#[test]
fn crate_rename_uses_the_aliased_path() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        #[ortho_config(crate = "my_cfg")]
        struct Nested {
            host: String,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains("implmy_cfg::RequiredConfigSchemaforNested"),
        "expected the aliased crate path: {tokens}"
    );
    Ok(())
}

#[test]
fn generic_structs_keep_declared_bounds() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Wrapper<T>
        where
            T: RequiredConfigSchema,
        {
            inner: T,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains("impl<T>"),
        "expected the declared generic parameter: {tokens}"
    );
    ensure!(
        tokens.contains("Wrapper<T>whereT:RequiredConfigSchema"),
        "expected the declared bound to survive: {tokens}"
    );
    Ok(())
}

#[test]
fn nested_fields_compose_the_child_schema() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            #[ortho_config(nested, skip_cli)]
            database: Database,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains(
            "<Databaseasortho_config::RequiredConfigSchema>::collect_required_schema(stack)?"
        ),
        "expected the child schema call: {tokens}"
    );
    ensure!(
        tokens.contains("SchemaChildren::Nested("),
        "expected the nested children variant: {tokens}"
    );
    ensure!(
        tokens.contains("Activation::Required"),
        "a plain nested object is required"
    );
    Ok(())
}

#[test]
fn optional_nested_parent_strips_wrappers_for_the_child_schema() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            #[ortho_config(nested, skip_cli)]
            database: Option<Box<Database>>,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains("<Databaseasortho_config::RequiredConfigSchema>"),
        "expected both wrappers stripped: {tokens}"
    );
    ensure!(
        tokens.contains("Activation::Optional"),
        "an optional parent deactivates its subtree"
    );
    Ok(())
}

#[test]
fn required_flatten_composes_the_child_requirements_unchanged() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            #[serde(flatten)]
            #[ortho_config(nested, skip_cli)]
            extra: Extra,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains(
            "fields.extend(<Extraasortho_config::RequiredConfigSchema>::collect_required_schema(stack)?);"
        ),
        "a required flatten keeps the child's activation: {tokens}"
    );
    ensure!(
        !tokens.contains("Activation::Optional"),
        "no child requirement is rewritten: {tokens}"
    );
    Ok(())
}

#[test]
fn optional_flatten_downgrades_the_child_requirements() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            #[serde(flatten)]
            #[ortho_config(nested, skip_cli)]
            extra: Option<Extra>,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains(concat!(
            "fields.extend(<Extraasortho_config::RequiredConfigSchema>",
            "::collect_required_schema(stack)?.into_iter().map(",
            "ortho_config::SchemaField::optional_subtree));"
        )),
        "an absent optional flatten must demote every composed requirement: {tokens}"
    );
    Ok(())
}

#[test]
fn defaulted_flatten_keeps_the_child_requirements() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            #[serde(flatten, default)]
            #[ortho_config(nested, skip_cli)]
            extra: Extra,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains(concat!(
            "fields.extend(<Extraasortho_config::RequiredConfigSchema>",
            "::collect_required_schema(stack)?);"
        )),
        "a defaulted flatten still deserializes the child: {tokens}"
    );
    ensure!(
        !tokens.contains("optional_subtree"),
        "a field default never rescues a flattened child: {tokens}"
    );
    Ok(())
}

#[rstest]
#[case("lowercase", "api_URL")]
#[case("UPPERCASE", "API_URL")]
#[case("PascalCase", "ApiURL")]
#[case("camelCase", "apiURL")]
#[case("snake_case", "api_URL")]
#[case("SCREAMING_SNAKE_CASE", "API_URL")]
#[case("kebab-case", "api-URL")]
#[case("SCREAMING-KEBAB-CASE", "API-URL")]
fn field_rename_rules_match_serde(#[case] rule: &str, #[case] expected: &str) -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(rename_all = #rule)]
        struct Root {
            api_URL: String,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains(&format!("key:::std::string::String::from(\"{expected}\")")),
        "`{rule}` must emit `{expected}` as Serde does: {tokens}"
    );
    Ok(())
}

#[test]
fn serde_naming_controls_keys_and_aliases() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        struct Nested {
            #[serde(alias = "alt_key")]
            api_key: String,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains(r#"key:::std::string::String::from("API_KEY")"#),
        "expected the deserialization rename rule: {tokens}"
    );
    ensure!(
        tokens.contains(r#"::std::string::String::from("alt_key")"#),
        "expected the alias: {tokens}"
    );
    Ok(())
}

#[test]
fn defaults_and_options_are_optional() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Nested {
            #[ortho_config(default = 8080)]
            port: u16,
            #[serde(default)]
            note: String,
            optional: Option<String>,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.matches("Activation::Optional").count() == 3,
        "expected three optional fields: {tokens}"
    );
    ensure!(
        !tokens.contains("Activation::Required"),
        "no field is required"
    );
    Ok(())
}

#[test]
fn container_default_makes_every_field_optional() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        #[serde(default)]
        struct Nested {
            host: String,
            port: u16,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.matches("Activation::Optional").count() == 2,
        "expected the container default to apply to every field: {tokens}"
    );
    Ok(())
}

#[rstest]
#[case(
    "#[serde(skip)] cache: String",
    "cache",
    "a skipped field is never deserialized"
)]
#[case(
    "#[serde(flatten)] extra: Extra",
    "extra",
    "an unmarked flatten has no readable key"
)]
fn fields_serde_never_reads_are_not_inspected(
    #[case] field: &str,
    #[case] key: &str,
    #[case] reason: &str,
) -> Result<()> {
    let input: DeriveInput = syn::parse_str(&format!("struct Nested {{ {field} }}"))?;
    let tokens = emit_plain(&input)?;
    ensure!(
        !tokens.contains(&format!(r#"String::from("{key}")"#)),
        "{reason}: {tokens}"
    );
    Ok(())
}

#[test]
fn cli_hints_are_emitted_only_for_matching_identifiers() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            port: u16,
            #[ortho_config(skip_cli)]
            secret: String,
            #[serde(rename = "ref")]
            reference: String,
        }
    };
    let tokens = emit(
        &input,
        &[
            hint("port", false),
            hint("secret", false),
            hint("reference", false),
        ],
    )?;
    ensure!(
        tokens.contains(r#"CliFlagHint{key:::std::string::String::from("port"),flag:::std::string::String::from("--port"),takes_value:true,}"#),
        "expected the matching field's hint: {tokens}"
    );
    ensure!(
        !tokens.contains(r#"String::from("--secret")"#),
        "a skipped CLI field has no flag: {tokens}"
    );
    ensure!(
        !tokens.contains(r#"String::from("--reference")"#),
        "a renamed key cannot be populated by the identifier-keyed parser: {tokens}"
    );
    Ok(())
}

#[test]
fn boolean_hints_do_not_take_a_value() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        struct Root {
            verbose: bool,
        }
    };
    let tokens = emit(&input, &[hint("verbose", true)])?;
    ensure!(
        tokens.contains(r#"flag:::std::string::String::from("--verbose"),takes_value:false,}"#),
        "a boolean flag takes no value: {tokens}"
    );
    Ok(())
}

#[test]
fn the_environment_prefix_is_recorded() -> Result<()> {
    let input: DeriveInput = syn::parse_quote! {
        #[ortho_config(prefix = "APP_")]
        struct Nested {
            host: String,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens
            .contains(r#"prefix:::std::option::Option::Some(::std::string::String::from("APP_"))"#),
        "expected the prefix: {tokens}"
    );
    Ok(())
}
