//! Tests for `#[ortho_config(behaviour(...))]` parsing and validation.

use super::super::*;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use syn::DeriveInput;

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

/// All four nested keys parse into their matching fields.
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

/// Declaring one key leaves the rest undeclared rather than defaulted.
///
/// The distinction matters downstream: `None` is what the policy check reports
/// as undeclared, so an omitted key must not quietly become a default value.
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
/// Each malformed declaration fails with a message naming the actual problem.
///
/// The cases cover an American spelling, a field-level placement, unknown
/// values for each keyed attribute, a malformed flag, and a misspelt nested
/// key. Asserting the message text keeps the diagnostics actionable.
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
/// Values outside the pinned flag grammar are rejected.
///
/// The cases cover a missing `--` prefix, uppercase, trailing punctuation, a
/// trailing separator, and an embedded space, each of which would otherwise
/// produce a flag an agent could not pass.
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

#[rstest]
#[case::within_one_group(
    r#"#[ortho_config(behaviour(interaction = "non_interactive", bypass = "--force"))]"#
)]
#[case::across_groups(concat!(
    r#"#[ortho_config(behaviour(interaction = "non_interactive"))]"#,
    "\n",
    r#"#[ortho_config(behaviour(bypass = "--force"))]"#
))]
#[case::across_groups_reversed(concat!(
    r#"#[ortho_config(behaviour(bypass = "--force"))]"#,
    "\n",
    r#"#[ortho_config(behaviour(interaction = "non_interactive"))]"#
))]
/// A bypass on a non-interactive command is always contradictory.
///
/// A command that never prompts has nothing to bypass, so this must be a hard
/// error rather than a warning (ADR-010). Because `behaviour(...)` may be
/// repeated, validation runs against the merged state; the contradiction must
/// therefore survive the keys being split across groups in either order, which
/// is what the ordering cases pin down.
fn rejects_non_interactive_with_bypass(#[case] attributes: &str) -> Result<()> {
    let source = format!("{attributes}\nstruct Demo {{ value: u8, }}");
    let input: DeriveInput = syn::parse_str(&source).context("failed to parse test input")?;
    let error = parse_input(&input)
        .err()
        .ok_or_else(|| anyhow::anyhow!("expected rejection; source: {source}"))?;
    ensure!(
        error.to_string().contains("contradictory behaviour"),
        "unexpected message: {error}"
    );
    Ok(())
}
