//! Tests for `#[ortho_config(output(...))]` parsing and validation.
//!
//! The parser is the strict half of the structured-output contract: unknown
//! keys, unknown tokens, wrong literal kinds, blank meanings, malformed class
//! names, and repeated leaves are all hard errors, because the generated IR has
//! no variant that could express an unchecked value. Each rejection therefore
//! asserts on the diagnostic text, so a failure for the wrong reason is caught
//! rather than counted as a pass.
//!
//! Class names are protocol identifiers and meanings are human explanations;
//! the two roles never share a helper, so a mixed-up argument cannot pass.

use super::super::doc_types::{ExitClassAttr, OutputAttrs};
use super::super::*;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use syn::DeriveInput;

/// A complete `output(...)` attribute body, exactly as it appears in the source.
#[derive(Clone, Copy)]
struct Declaration<'a>(&'a str);

/// A diagnostic substring that a rejected declaration must contain.
#[derive(Clone, Copy)]
struct Needle<'a>(&'a str);

/// Parses a struct carrying only the given `output(...)` declaration.
fn parse_output(decl: Declaration<'_>) -> Result<OutputAttrs> {
    let source = format!("#[ortho_config({})]\nstruct Demo {{ value: u8, }}", decl.0);
    let input: DeriveInput = syn::parse_str(&source).context("failed to parse test input")?;
    let (_, _, struct_attrs, _) = parse_input(&input).context("parse_input failed")?;
    struct_attrs
        .doc
        .output
        .ok_or_else(|| anyhow::anyhow!("expected output attrs"))
}

/// Asserts that the declaration is rejected with a message containing `needle`.
fn assert_rejected(decl: Declaration<'_>, needle: Needle<'_>) {
    let Err(error) = parse_output(decl) else {
        panic!("expected `{}` to be rejected, but it parsed", decl.0);
    };
    let message = format!("{error:#}");
    assert!(
        message.contains(needle.0),
        "expected rejection mentioning {:?}, got: {message}",
        needle.0
    );
}

/// Renders a class for assertion messages.
fn class_name(class: &ExitClassAttr) -> String {
    format!("{}: {}", class.name, class.meaning)
}

/// A fully declared block parses every leaf and both classes.
#[test]
fn parses_fully_declared_block() -> Result<()> {
    let output = parse_output(Declaration(
        r#"output(
            supports_json = true,
            stdout = "data",
            stderr_diagnostics = false,
            exit_class(name = "success", meaning = "The command completed"),
            exit_class(name = "usage_error", meaning = "The invocation was invalid")
        )"#,
    ))?;

    ensure!(
        output.supports_json == Some(true),
        "JSON support was declared"
    );
    ensure!(
        output.stdout.as_deref() == Some("data"),
        "stdout purpose was declared"
    );
    ensure!(
        output.stderr_diagnostics == Some(false),
        "a declared refusal must survive as `Some(false)`, not be dropped"
    );
    let classes = output
        .exit_classes
        .iter()
        .map(class_name)
        .collect::<Vec<_>>();
    ensure!(
        classes
            == [
                "success: The command completed",
                "usage_error: The invocation was invalid"
            ],
        "classes should keep declaration order, got: {classes:?}"
    );
    Ok(())
}

/// `output()` is an explicitly empty declaration that claims nothing.
#[test]
fn accepts_empty_group() -> Result<()> {
    let output = parse_output(Declaration("output()"))?;

    ensure!(output.supports_json.is_none(), "JSON support is undeclared");
    ensure!(output.stdout.is_none(), "stdout purpose is undeclared");
    ensure!(
        output.stderr_diagnostics.is_none(),
        "diagnostic stderr is undeclared"
    );
    ensure!(
        output.exit_classes.is_empty(),
        "no exit classes were declared"
    );
    Ok(())
}

/// Distinct leaves may come from separate groups, as the renderer group allows.
#[test]
fn composes_leaves_across_groups() -> Result<()> {
    let output = parse_output(Declaration(
        r#"output(supports_json = true), output(stdout = "summary")"#,
    ))?;

    ensure!(
        output.supports_json == Some(true),
        "first group contributed"
    );
    ensure!(
        output.stdout.as_deref() == Some("summary"),
        "second group contributed"
    );
    Ok(())
}

/// Every accepted stdout token reaches the model unchanged.
#[rstest]
#[case::data("data")]
#[case::path("path")]
#[case::summary("summary")]
#[case::none("none")]
fn accepts_each_stdout_token(#[case] token: &str) -> Result<()> {
    let declaration = format!(r#"output(stdout = "{token}")"#);
    let output = parse_output(Declaration(&declaration))?;

    ensure!(
        output.stdout.as_deref() == Some(token),
        "expected stdout {token:?}, got {:?}",
        output.stdout
    );
    Ok(())
}

/// Bare booleans are accepted as `true`, mirroring the renderer group.
#[test]
fn accepts_bare_boolean_as_true() -> Result<()> {
    let output = parse_output(Declaration("output(supports_json)"))?;

    ensure!(
        output.supports_json == Some(true),
        "a bare key should mean `true`"
    );
    Ok(())
}

/// Rejected declarations, each paired with the diagnostic it must produce.
#[rstest]
#[case::unknown_key(r#"output(schema = "v1")"#, "unknown output attribute `schema`")]
#[case::unknown_stdout_token(r#"output(stdout = "json")"#, "unknown stdout 'json'")]
#[case::stdout_token_is_case_sensitive(r#"output(stdout = "Data")"#, "unknown stdout 'Data'")]
#[case::non_boolean_support(r#"output(supports_json = "yes")"#, "expected")]
#[case::non_boolean_diagnostics(r#"output(stderr_diagnostics = "true")"#, "expected")]
#[case::bare_stdout_token("output(stdout = data)", "expected literal")]
#[case::repeated_stdout(
    r#"output(stdout = "data", stdout = "none")"#,
    "duplicate output policy `stdout`"
)]
#[case::repeated_support_across_groups(
    "output(supports_json), output(supports_json = false)",
    "duplicate output policy `supports_json`"
)]
#[case::class_without_name(
    r#"output(exit_class(meaning = "The command completed"))"#,
    "requires `name"
)]
#[case::class_without_meaning(r#"output(exit_class(name = "success"))"#, "requires `meaning")]
#[case::class_name_is_uppercase(
    r#"output(exit_class(name = "Success", meaning = "The command completed"))"#,
    "invalid exit class name 'Success'"
)]
#[case::class_name_has_punctuation(
    r#"output(exit_class(name = "usage-error", meaning = "The invocation was invalid"))"#,
    "invalid exit class name 'usage-error'"
)]
#[case::class_name_starts_with_digit(
    r#"output(exit_class(name = "1usage", meaning = "The invocation was invalid"))"#,
    "invalid exit class name '1usage'"
)]
#[case::class_name_is_empty(
    r#"output(exit_class(name = "", meaning = "The invocation was invalid"))"#,
    "invalid exit class name ''"
)]
#[case::blank_meaning(
    r#"output(exit_class(name = "success", meaning = "   "))"#,
    "must not be blank"
)]
#[case::non_string_meaning(
    r#"output(exit_class(name = "success", meaning = 7))"#,
    "exit class `meaning` must be a string literal"
)]
#[case::non_string_name(
    r#"output(exit_class(name = 1, meaning = "The command completed"))"#,
    "exit class `name` must be a string literal"
)]
#[case::repeated_class_member(
    r#"output(exit_class(name = "success", meaning = "a", name = "other"))"#,
    "duplicate exit class member `name`"
)]
#[case::unknown_class_member(
    r#"output(exit_class(name = "success", meaning = "a", severity = "high"))"#,
    "unknown exit class member `severity`"
)]
#[case::duplicate_class_across_entries(
    r#"output(
        exit_class(name = "success", meaning = "The command completed"),
        exit_class(name = "success", meaning = "A different explanation")
    )"#,
    "duplicate exit class 'success'"
)]
fn rejects_invalid_declarations(#[case] declaration: &str, #[case] needle: &str) {
    assert_rejected(Declaration(declaration), Needle(needle));
}

/// A class name that merely resembles a declared one is still distinct, so the
/// duplicate check never normalizes case or punctuation into a match.
#[test]
fn distinct_class_names_are_accepted() -> Result<()> {
    let output = parse_output(Declaration(
        r#"output(
            exit_class(name = "usage", meaning = "The invocation was invalid"),
            exit_class(name = "usage_error", meaning = "The invocation was invalid")
        )"#,
    ))?;

    ensure!(
        output.exit_classes.len() == 2,
        "both classes should be kept, got {}",
        output.exit_classes.len()
    );
    Ok(())
}
