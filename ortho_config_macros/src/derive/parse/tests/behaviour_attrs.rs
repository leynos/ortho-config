//! Tests for `#[ortho_config(behaviour(...))]` parsing and validation.

use super::super::*;
use anyhow::{Context, Result, ensure};
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

#[test]
fn rejects_en_us_spelling_at_struct_level() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(behavior(interaction = "interactive"))]
        struct Demo {
            value: u8,
        }
    };
    let err = parse_input(&input).err().expect("expected rejection");
    let msg = err.to_string();
    ensure!(
        msg.contains("en-GB spelling `behaviour`"),
        "unexpected message: {msg}"
    );
    Ok(())
}

#[test]
fn rejects_behaviour_at_field_level() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(prefix = "X")]
        struct Demo {
            #[ortho_config(behaviour(mutation = "write"))]
            value: u8,
        }
    };
    let err = parse_input(&input).err().expect("expected rejection");
    let msg = err.to_string();
    ensure!(
        msg.contains("struct-level attribute"),
        "unexpected message: {msg}"
    );
    Ok(())
}

#[test]
fn rejects_invalid_interaction_value() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(behaviour(interaction = "sometimes"))]
        struct Demo {
            value: u8,
        }
    };
    let err = parse_input(&input).err().expect("expected rejection");
    let msg = err.to_string();
    ensure!(
        msg.contains("unknown interaction 'sometimes'"),
        "unexpected message: {msg}"
    );
    Ok(())
}

#[test]
fn rejects_invalid_mutation_value() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(behaviour(mutation = "destroy"))]
        struct Demo {
            value: u8,
        }
    };
    let err = parse_input(&input).err().expect("expected rejection");
    let msg = err.to_string();
    ensure!(
        msg.contains("unknown mutation 'destroy'"),
        "unexpected message: {msg}"
    );
    Ok(())
}

#[test]
fn rejects_bad_bypass_grammar() -> Result<()> {
    for bad in ["force", "--Force", "--force!", "--force--", "x --force"] {
        let input: DeriveInput = parse_quote! {
            #[ortho_config(behaviour(bypass = "force"))]
            struct Demo {
                value: u8,
            }
        };
        let err = parse_input(&input).err().expect("expected rejection");
        ensure!(
            err.to_string()
                .contains("flags must match --[a-z0-9]+(-[a-z0-9]+)*"),
            "bad bypass value {bad:?} gave: {err}"
        );
    }
    Ok(())
}

#[test]
fn rejects_bad_dry_run_grammar() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(behaviour(dry_run = "dry_run"))]
        struct Demo {
            value: u8,
        }
    };
    let err = parse_input(&input).err().expect("expected rejection");
    ensure!(
        err.to_string().contains("flags must match"),
        "unexpected message: {err}"
    );
    Ok(())
}

#[test]
fn rejects_unknown_nested_key() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(behaviour(interation = "interactive"))]
        struct Demo {
            value: u8,
        }
    };
    let err = parse_input(&input).err().expect("expected rejection");
    let msg = err.to_string();
    ensure!(
        msg.contains("unknown behaviour attribute"),
        "unexpected message: {msg}"
    );
    Ok(())
}

#[test]
fn rejects_non_interactive_with_bypass() -> Result<()> {
    let input: DeriveInput = parse_quote! {
        #[ortho_config(behaviour(
            interaction = "non_interactive",
            bypass = "--force"
        ))]
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
