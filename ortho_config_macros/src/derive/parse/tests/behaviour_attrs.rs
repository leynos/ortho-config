//! Tests for `#[ortho_config(behaviour(...))]` parsing and validation.

use super::super::*;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use syn::{DeriveInput, parse_quote};

/// Parse a struct carrying only the given `behaviour(...)` declaration.
fn parse_behaviour(decl: &str) -> Result<BehaviourAttrs> {
    let source = format!("#[ortho_config({decl})]\nstruct Demo {{ value: u8, }}");
    let input: DeriveInput = syn::parse_str(&source).context("failed to parse test input")?;
    let (_, _, struct_attrs, _) = parse_input(&input).context("parse_input failed")?;
    struct_attrs
        .doc
        .behaviour
        .ok_or_else(|| anyhow::anyhow!("expected behaviour attrs"))
}

#[test]
fn parses_fully_declared_behaviour() -> Result<()> {
    let b = parse_behaviour(
        r#"behaviour(
        interaction = "interactive",
        mutation = "delete",
        bypass = "--force",
        dry_run = "--dry-run"
    )"#,
    )?;
    ensure!(
        b.interaction.as_deref() == Some("interactive"),
        "interaction: {:?}",
        b.interaction
    );
    ensure!(
        b.mutation.as_deref() == Some("delete"),
        "mutation: {:?}",
        b.mutation
    );
    ensure!(
        b.bypass.as_deref() == Some("--force"),
        "bypass: {:?}",
        b.bypass
    );
    ensure!(
        b.dry_run.as_deref() == Some("--dry-run"),
        "dry_run: {:?}",
        b.dry_run
    );
    Ok(())
}

#[test]
fn parses_partial_behaviour() -> Result<()> {
    let b = parse_behaviour(r#"behaviour(interaction = "non_interactive")"#)?;
    ensure!(b.interaction.as_deref() == Some("non_interactive"));
    ensure!(b.mutation.is_none(), "expected undeclared mutation");
    ensure!(b.bypass.is_none(), "expected undeclared bypass");
    ensure!(b.dry_run.is_none(), "expected undeclared dry_run");
    Ok(())
}

/// A rejection scenario for invalid `behaviour(...)` declarations.
struct InvalidBehaviourCase {
    source: &'static str,
    expected_substring: &'static str,
}

#[rstest]
#[case::behaviour_en_us_spelling(InvalidBehaviourCase {
    source: r#"
        #[ortho_config(behavior(interaction = "interactive"))]
        struct Demo {
            value: u8,
        }
    "#,
    expected_substring: "en-GB spelling `behaviour`",
})]
#[case::behaviour_on_field(InvalidBehaviourCase {
    source: r#"
        #[ortho_config(prefix = "X")]
        struct Demo {
            #[ortho_config(behaviour(mutation = "write"))]
            value: u8,
        }
    "#,
    expected_substring: "struct-level attribute",
})]
#[case::invalid_interaction(InvalidBehaviourCase {
    source: r#"
        #[ortho_config(behaviour(interaction = "sometimes"))]
        struct Demo {
            value: u8,
        }
    "#,
    expected_substring: "unknown interaction 'sometimes'",
})]
#[case::invalid_mutation(InvalidBehaviourCase {
    source: r#"
        #[ortho_config(behaviour(mutation = "destroy"))]
        struct Demo {
            value: u8,
        }
    "#,
    expected_substring: "unknown mutation 'destroy'",
})]
#[case::bad_dry_run(InvalidBehaviourCase {
    source: r#"
        #[ortho_config(behaviour(dry_run = "dry_run"))]
        struct Demo {
            value: u8,
        }
    "#,
    expected_substring: "flags must match",
})]
#[case::unknown_nested_key(InvalidBehaviourCase {
    source: r#"
        #[ortho_config(behaviour(interation = "interactive"))]
        struct Demo {
            value: u8,
        }
    "#,
    expected_substring: "unknown behaviour attribute",
})]
#[case::non_interactive_with_bypass(InvalidBehaviourCase {
    source: r#"
        #[ortho_config(behaviour(
            interaction = "non_interactive",
            bypass = "--force"
        ))]
        struct Demo {
            value: u8,
        }
    "#,
    expected_substring: "contradictory behaviour",
})]
fn rejects_invalid_behaviour_declarations(#[case] case: InvalidBehaviourCase) -> Result<()> {
    let input: DeriveInput = syn::parse_str(case.source).context("failed to parse test input")?;
    let error = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow::anyhow!("expected rejection; source: {}", case.source))?;
    let message = error.to_string();
    ensure!(
        message.contains(case.expected_substring),
        "expected substring {:?} not found in: {message}",
        case.expected_substring
    );
    Ok(())
}

#[rstest]
#[case("force")]
#[case("--Force")]
#[case("--force!")]
#[case("--force--")]
#[case("x --force")]
fn rejects_bad_bypass_grammar(#[case] bypass: &str) -> Result<()> {
    let source = format!(
        r#"
        #[ortho_config(behaviour(bypass = "{bypass}"))]
        struct Demo {{
            value: u8,
        }}
        "#
    );
    let input: DeriveInput = syn::parse_str(&source).context("failed to parse test input")?;
    let error = parse_input(&input).err().ok_or_else(|| {
        anyhow::anyhow!("expected rejection for bypass {bypass:?}; source: {source}")
    })?;
    ensure!(
        error
            .to_string()
            .contains("flags must match --[a-z0-9]+(-[a-z0-9]+)*"),
        "bad bypass value {bypass:?} gave: {error}"
    );
    Ok(())
}

#[test]
fn rejects_non_interactive_with_bypass_split_across_groups() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(behaviour(interaction = "non_interactive"))]
        #[ortho_config(behaviour(bypass = "--force"))]
        struct Demo {
            value: u8,
        }
    };
    let err = parse_input(&input).err().expect("expected rejection");
    let msg = err.to_string();
    ensure!(
        msg.contains("contradictory behaviour"),
        "unexpected message: {msg}"
    );
    Ok(())
}

#[test]
fn rejects_non_interactive_with_bypass_split_across_groups_reversed() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(behaviour(bypass = "--force"))]
        #[ortho_config(behaviour(interaction = "non_interactive"))]
        struct Demo {
            value: u8,
        }
    };
    let err = parse_input(&input).err().expect("expected rejection");
    let msg = err.to_string();
    ensure!(
        msg.contains("contradictory behaviour"),
        "unexpected message: {msg}"
    );
    Ok(())
}
