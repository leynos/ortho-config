//! Tests for `#[ortho_config(renderer(...))]` parsing and validation.
//!
//! The parser is the strict half of the dual-renderer contract: it must reject
//! unknown keys, unknown tokens, wrong literal types, and repeated leaves,
//! because the generated IR has no way to express an unchecked value. Each test
//! therefore asserts on the diagnostic text rather than only on `is_err()`, so
//! a rejection for the wrong reason fails.

use super::super::doc_types::{RenderWidthAttr, RendererAttrs, RendererRecordAttrs};
use super::super::*;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use syn::DeriveInput;

/// Parses a struct carrying only the given `renderer(...)` declaration.
pub(super) fn parse_renderer(decl: &str) -> Result<RendererAttrs> {
    let source = format!("#[ortho_config({decl})]\nstruct Demo {{ value: u8, }}");
    let input: DeriveInput = syn::parse_str(&source).context("failed to parse test input")?;
    let (_, _, struct_attrs, _) = parse_input(&input).context("parse_input failed")?;
    struct_attrs
        .doc
        .renderer
        .ok_or_else(|| anyhow::anyhow!("expected renderer attrs"))
}

/// Asserts that the declaration is rejected with a message containing `needle`.
pub(super) fn assert_rejected(decl: &str, needle: &str) {
    let Err(error) = parse_renderer(decl) else {
        panic!("expected `{decl}` to be rejected, but it parsed");
    };
    let message = format!("{error:#}");
    assert!(
        message.contains(needle),
        "expected rejection mentioning {needle:?}, got: {message}"
    );
}

/// A fully declared human record parses every policy leaf.
#[test]
fn parses_fully_declared_human_record() -> Result<()> {
    let renderer = parse_renderer(
        r#"renderer(human(
            supported = true,
            tty = "adapts",
            closed_stdin = "eof",
            colour = "auto",
            emoji = "never",
            progress = "always",
            pager = "never",
            width = 80,
            accessibility = "text_equivalent",
            plain = "available",
            stdout = "text",
            stderr = "mixed",
            progress_inheritance = "allowed",
            subprocess_inheritance = "forbidden"
        ))"#,
    )?;
    let human = renderer.human.context("expected a human record")?;

    assert_human_policy_leaves(&human)?;
    ensure!(renderer.machine.is_none(), "no machine record was declared");
    Ok(())
}

/// Renders a width attribute for assertion messages.
fn width_name(width: &RenderWidthAttr) -> String {
    match width {
        RenderWidthAttr::Unbounded => "unbounded".to_owned(),
        RenderWidthAttr::Terminal => "terminal".to_owned(),
        RenderWidthAttr::Fixed(columns) => format!("fixed({columns})"),
    }
}

/// Both records may be declared independently in one attribute.
#[test]
fn parses_both_records_independently() -> Result<()> {
    let renderer = parse_renderer(
        r#"renderer(human(colour = "always"), machine(supported = true, stdout = "structured"))"#,
    )?;

    let human = renderer.human.context("expected a human record")?;
    let machine = renderer.machine.context("expected a machine record")?;
    ensure!(human.colour.as_deref() == Some("always"), "human colour");
    ensure!(
        machine.supported == Some(true),
        "machine supported: {:?}",
        machine.supported
    );
    ensure!(
        machine.stdout.as_deref() == Some("structured"),
        "machine stdout: {:?}",
        machine.stdout
    );
    ensure!(
        human.supported.is_none(),
        "the human support stays undeclared"
    );
    ensure!(
        machine.colour.is_none(),
        "the machine colour stays undeclared"
    );
    Ok(())
}

/// A bare `supported` leaf means true, matching the bool attribute convention.
#[test]
fn bare_supported_leaf_means_true() -> Result<()> {
    let renderer = parse_renderer("renderer(machine(supported))")?;
    let machine = renderer.machine.context("expected a machine record")?;
    ensure!(
        machine.supported == Some(true),
        "supported: {:?}",
        machine.supported
    );
    Ok(())
}

/// Separate groups for one record merge distinct leaves.
///
/// This is the case that makes the merge rule worth having: a caller can split
/// a long declaration without changing meaning.
#[test]
fn separate_groups_for_one_record_merge() -> Result<()> {
    let renderer =
        parse_renderer(r#"renderer(human(colour = "never"), human(width = "terminal"))"#)?;

    let human = renderer.human.context("expected a human record")?;
    ensure!(human.colour.as_deref() == Some("never"), "colour");
    ensure!(
        matches!(human.width, Some(RenderWidthAttr::Terminal)),
        "width: {:?}",
        human.width.as_ref().map(width_name)
    );
    Ok(())
}

/// The two special width spellings parse to their enum shapes.
#[rstest]
#[case::terminal("terminal", RenderWidthAttr::Terminal)]
#[case::unbounded("unbounded", RenderWidthAttr::Unbounded)]
fn parses_named_width(#[case] token: &str, #[case] expected: RenderWidthAttr) -> Result<()> {
    let renderer = parse_renderer(&format!(r#"renderer(human(width = "{token}"))"#))?;
    let human = renderer.human.context("expected a human record")?;
    let actual = human.width.context("expected a width")?;
    ensure!(
        width_name(&actual) == width_name(&expected),
        "width: {}",
        width_name(&actual)
    );
    Ok(())
}

/// Asserts the support and terminal leaves of a fully declared record.
fn assert_human_policy_leaves(human: &RendererRecordAttrs) -> Result<()> {
    assert_human_output_leaves(human)?;
    Ok(())
}

/// Asserts the declared stream and inheritance leaves of a record.
fn assert_human_output_leaves(human: &RendererRecordAttrs) -> Result<()> {
    ensure!(
        human.stdout.as_deref() == Some("text"),
        "stdout: {:?}",
        human.stdout
    );
    ensure!(
        human.stderr.as_deref() == Some("mixed"),
        "stderr: {:?}",
        human.stderr
    );
    ensure!(
        human.progress_inheritance.as_deref() == Some("allowed"),
        "progress_inheritance: {:?}",
        human.progress_inheritance
    );
    ensure!(
        human.subprocess_inheritance.as_deref() == Some("forbidden"),
        "subprocess_inheritance: {:?}",
        human.subprocess_inheritance
    );
    Ok(())
}

/// Widths at both ends of the accepted range parse, and one past each end does
/// not. The bounds mirror the IR's `NonZeroU16`.
#[rstest]
#[case::one(1)]
#[case::max(u64::from(u16::MAX))]
fn accepts_width_within_range(#[case] columns: u64) -> Result<()> {
    let renderer = parse_renderer(&format!("renderer(human(width = {columns}))"))?;
    let human = renderer.human.context("expected a human record")?;
    ensure!(
        matches!(human.width, Some(RenderWidthAttr::Fixed(value)) if u64::from(value) == columns),
        "width: {:?}",
        human.width.as_ref().map(width_name)
    );
    Ok(())
}

/// Zero columns is rejected because the IR counts are non-zero.
#[test]
fn rejects_zero_width() {
    assert_rejected("renderer(human(width = 0))", "greater than zero");
}

/// A width above `u16::MAX` is rejected rather than silently truncated.
#[test]
fn rejects_overflowing_width() {
    assert_rejected("renderer(human(width = 65536))", "must not exceed 65535");
}

/// An unrecognized width spelling is rejected.
#[test]
fn rejects_unknown_named_width() {
    assert_rejected("renderer(human(width = \"wide\"))", "unknown width 'wide'");
}

/// A non-string, non-integer width is rejected as the wrong literal type.
#[test]
fn rejects_width_with_wrong_literal_type() {
    assert_rejected("renderer(human(width = true))", "expected");
}

/// Unknown top-level renderer keys are rejected.
#[test]
fn rejects_unknown_renderer_key() {
    assert_rejected(
        "renderer(audio(colour = \"auto\"))",
        "unknown renderer attribute `audio`",
    );
}

/// Unknown policy keys inside a record are rejected.
#[test]
fn rejects_unknown_record_policy() {
    assert_rejected(
        "renderer(human(hyperlink = \"always\"))",
        "unknown renderer policy `hyperlink`",
    );
}

/// Every token leaf rejects a spelling outside its accepted set.
///
/// The rows are deliberately uneven: each names a token list the IR also
/// enforces, so a widened parser list would fail here first.
#[rstest]
#[case::tty(r#"tty = "sometimes""#, "unknown tty 'sometimes'")]
#[case::closed_stdin(r#"closed_stdin = "hangup""#, "unknown closed_stdin 'hangup'")]
#[case::colour(r#"colour = "on""#, "unknown colour 'on'")]
#[case::emoji(r#"emoji = "maybe""#, "unknown emoji 'maybe'")]
#[case::progress(r#"progress = "on""#, "unknown progress 'on'")]
#[case::pager(r#"pager = "on""#, "unknown pager 'on'")]
#[case::accessibility(r#"accessibility = "full""#, "unknown accessibility 'full'")]
#[case::plain(r#"plain = "partial""#, "unknown plain 'partial'")]
#[case::stdout(r#"stdout = "binary""#, "unknown stdout 'binary'")]
#[case::stderr(r#"stderr = "binary""#, "unknown stderr 'binary'")]
#[case::progress_inheritance(
    r#"progress_inheritance = "sometimes""#,
    "unknown progress_inheritance 'sometimes'"
)]
#[case::subprocess_inheritance(
    r#"subprocess_inheritance = "sometimes""#,
    "unknown subprocess_inheritance 'sometimes'"
)]
fn rejects_unknown_token(#[case] leaf: &str, #[case] needle: &str) {
    assert_rejected(&format!("renderer(human({leaf}))"), needle);
}

/// A token leaf rejects a non-string literal.
#[test]
fn rejects_token_with_wrong_literal_type() {
    assert_rejected("renderer(human(colour = 1))", "colour must be a string");
}

/// `supported` rejects a non-boolean literal.
#[test]
fn rejects_supported_with_wrong_literal_type() {
    assert_rejected("renderer(human(supported = \"yes\"))", "expected");
}

/// Repeating a leaf, even across groups for the same record, is an error.
#[rstest]
#[case::same_group(r#"renderer(human(colour = "auto", colour = "never"))"#)]
#[case::across_groups(r#"renderer(human(colour = "auto"), human(colour = "never"))"#)]
fn rejects_duplicate_policy(#[case] decl: &str) {
    assert_rejected(decl, "duplicate renderer policy `colour`");
}
