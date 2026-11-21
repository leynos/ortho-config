//! Key-model decisions the generator copies from Serde's own attributes.
//!
//! The schema and the deserializer must agree about which keys a struct
//! reads: a renamed key, an alias, and a field Serde never reads at all each
//! change what can be reported as missing. These cases pin the emitted key
//! text, so a divergence from Serde's rules fails here rather than as a
//! wrong diagnostic at run time.

use anyhow::{Result, ensure};
use rstest::rstest;
use syn::DeriveInput;

use super::emit_plain;

/// One `rename_all` spelling and the key it must produce.
///
/// The pair travels together because a rule without its expectation asserts
/// nothing; grouping them keeps each case to a single argument.
struct RenameCase {
    /// The `#[serde(rename_all = "...")]` value under test.
    rule: &'static str,
    /// The key Serde derives for the fixture's `api_URL` field.
    expected: &'static str,
}

impl RenameCase {
    /// Describe one rename rule and the key it must emit.
    const fn new(rule: &'static str, expected: &'static str) -> Self {
        Self { rule, expected }
    }
}

/// One field Serde never reads, and the key it must not contribute.
struct UnreadField {
    /// The field declaration the generator is given.
    declared: &'static str,
    /// The key that must not appear for that field.
    key: &'static str,
    /// Why Serde never reads the field.
    reason: &'static str,
}

#[rstest]
#[case(RenameCase::new("lowercase", "api_URL"))]
#[case(RenameCase::new("UPPERCASE", "API_URL"))]
#[case(RenameCase::new("PascalCase", "ApiURL"))]
#[case(RenameCase::new("camelCase", "apiURL"))]
#[case(RenameCase::new("snake_case", "api_URL"))]
#[case(RenameCase::new("SCREAMING_SNAKE_CASE", "API_URL"))]
#[case(RenameCase::new("kebab-case", "api-URL"))]
#[case(RenameCase::new("SCREAMING-KEBAB-CASE", "API-URL"))]
fn field_rename_rules_match_serde(#[case] case: RenameCase) -> Result<()> {
    let rule = case.rule;
    let input: DeriveInput = syn::parse_quote! {
        #[serde(rename_all = #rule)]
        struct Root {
            api_URL: String,
        }
    };
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains(&format!(
            "key:::std::string::String::from(\"{}\")",
            case.expected
        )),
        "`{}` must emit `{}` as Serde does: {tokens}",
        case.rule,
        case.expected
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

#[rstest]
#[case(UnreadField {
    declared: "#[serde(skip)] cache: String",
    key: "cache",
    reason: "a skipped field is never deserialized",
})]
#[case(UnreadField {
    declared: "#[serde(flatten)] extra: Extra",
    key: "extra",
    reason: "an unmarked flatten has no readable key",
})]
fn fields_serde_never_reads_are_not_inspected(#[case] case: UnreadField) -> Result<()> {
    let field = case.declared;
    let input: DeriveInput = syn::parse_str(&format!("struct Nested {{ {field} }}"))?;
    let tokens = emit_plain(&input)?;
    ensure!(
        !tokens.contains(&format!(r#"String::from("{}")"#, case.key)),
        "{}: {tokens}",
        case.reason
    );
    Ok(())
}

/// A transparent container has no keys of its own to contribute.
///
/// `#[serde(transparent)]` reads and writes the single inner value from the
/// parent's key, so the field names this type records belong to that inner
/// value, not to the object a parent field holds. The schema therefore emits no
/// keyed child and answers `declares_nested_object() == false`, which keeps the
/// parent field an opaque leaf. The plain case is the control: the same fields
/// are recorded when the attribute is absent, so the difference is the
/// attribute's effect rather than the fixture's shape.
#[rstest]
#[case::transparent("#[serde(transparent)]", false)]
#[case::plain("", true)]
fn a_transparent_container_declares_no_nested_object(
    #[case] attr: &str,
    #[case] composes: bool,
) -> Result<()> {
    let input: DeriveInput = syn::parse_str(&format!("{attr} struct Wrapper {{ inner: String }}"))?;
    let tokens = emit_plain(&input)?;
    ensure!(
        tokens.contains("fndeclares_nested_object()->bool{false}") != composes,
        "expected the nested-object answer to follow the attribute: {tokens}"
    );
    ensure!(
        tokens.contains(r#"String::from("inner")"#) == composes,
        "a transparent container must record no keyed child: {tokens}"
    );
    Ok(())
}
