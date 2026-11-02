//! Tests for `#[ortho_config(renderer(...))]` parsing and validation.
//!
//! The parser is the strict half of the dual-renderer contract: it must reject
//! unknown keys, unknown tokens, wrong literal types, and repeated leaves,
//! because the generated IR has no way to express an unchecked value. Each test
//! therefore asserts on the diagnostic text rather than only on `is_err()`, so
//! a rejection for the wrong reason fails.
//!
//! The three string roles are distinct — a declaration handed to the parser, a
//! leaf fragment spliced into one, and a diagnostic substring — so each gets
//! its own newtype rather than being passed around as a bare `&str`.

use super::super::doc_types::{RenderWidthAttr, RendererAttrs, RendererRecordAttrs};
use super::super::*;
use anyhow::{Context, Result, ensure};
use rstest::rstest;
use syn::DeriveInput;

/// A complete `renderer(...)` body, exactly as it appears in the attribute.
#[derive(Clone, Copy)]
pub(super) struct Declaration<'a>(pub(super) &'a str);

/// One `key = "token"` leaf, as it is written inside a record group.
#[derive(Clone, Copy)]
pub(super) struct PolicyLeaf<'a>(pub(super) &'a str);

/// A diagnostic substring that a rejected declaration must contain.
#[derive(Clone, Copy)]
pub(super) struct Needle<'a>(pub(super) &'a str);

impl PolicyLeaf<'_> {
    /// Splices the leaf into its own human record so the parser can see it.
    ///
    /// The surrounding declaration is the minimum that reaches policy-leaf
    /// validation, so its shape lives here rather than in each test.
    fn in_human_record(self) -> String {
        format!("renderer(human({}))", self.0)
    }
}

/// Parses a struct carrying only the given `renderer(...)` declaration.
pub(super) fn parse_renderer(decl: Declaration<'_>) -> Result<RendererAttrs> {
    let source = format!("#[ortho_config({})]\nstruct Demo {{ value: u8, }}", decl.0);
    let input: DeriveInput = syn::parse_str(&source).context("failed to parse test input")?;
    let (_, _, struct_attrs, _) = parse_input(&input).context("parse_input failed")?;
    struct_attrs
        .doc
        .renderer
        .ok_or_else(|| anyhow::anyhow!("expected renderer attrs"))
}

/// Asserts that the declaration is rejected with a message containing `needle`.
pub(super) fn assert_rejected(decl: Declaration<'_>, needle: Needle<'_>) {
    let Err(error) = parse_renderer(decl) else {
        panic!("expected `{}` to be rejected, but it parsed", decl.0);
    };
    let message = format!("{error:#}");
    assert!(
        message.contains(needle.0),
        "expected rejection mentioning {:?}, got: {message}",
        needle.0
    );
}

/// A fully declared human record parses every policy leaf.
#[test]
fn parses_fully_declared_human_record() -> Result<()> {
    let renderer = parse_renderer(Declaration(
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
    ))?;
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
    let renderer = parse_renderer(Declaration(
        r#"renderer(human(colour = "always"), machine(supported = true, stdout = "structured"))"#,
    ))?;

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
    let renderer = parse_renderer(Declaration("renderer(machine(supported))"))?;
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
    let renderer = parse_renderer(Declaration(
        r#"renderer(human(colour = "never"), human(width = "terminal"))"#,
    ))?;

    let human = renderer.human.context("expected a human record")?;
    ensure!(human.colour.as_deref() == Some("never"), "colour");
    ensure!(
        matches!(human.width, Some(RenderWidthAttr::Terminal)),
        "width: {:?}",
        human.width.as_ref().map(width_name)
    );
    Ok(())
}

/// The two special width spellings round-trip through the parser.
///
/// Deriving the declared token from the expected shape keeps the accepted
/// spelling and the assertion message in one place, so they cannot drift.
#[rstest]
#[case::terminal(RenderWidthAttr::Terminal)]
#[case::unbounded(RenderWidthAttr::Unbounded)]
fn parses_named_width(#[case] expected: RenderWidthAttr) -> Result<()> {
    let declared = width_name(&expected);
    let renderer = parse_renderer(Declaration(&format!(
        r#"renderer(human(width = "{declared}"))"#
    )))?;
    let human = renderer.human.context("expected a human record")?;
    let actual = human.width.context("expected a width")?;
    ensure!(
        width_name(&actual) == width_name(&expected),
        "width: {}",
        width_name(&actual)
    );
    Ok(())
}

/// Asserts every leaf of the fully declared human record.
///
/// Each accepted token is declared with a distinct value, so a parser that
/// stored one leaf in another leaf's slot cannot satisfy this helper.
fn assert_human_policy_leaves(human: &RendererRecordAttrs) -> Result<()> {
    ensure!(
        human.supported == Some(true),
        "supported: {:?}",
        human.supported
    );
    ensure!(
        human.tty.as_deref() == Some("adapts"),
        "tty: {:?}",
        human.tty
    );
    ensure!(
        human.closed_stdin.as_deref() == Some("eof"),
        "closed_stdin: {:?}",
        human.closed_stdin
    );
    ensure!(
        human.colour.as_deref() == Some("auto"),
        "colour: {:?}",
        human.colour
    );
    ensure!(
        human.emoji.as_deref() == Some("never"),
        "emoji: {:?}",
        human.emoji
    );
    ensure!(
        human.progress.as_deref() == Some("always"),
        "progress: {:?}",
        human.progress
    );
    ensure!(
        human.pager.as_deref() == Some("never"),
        "pager: {:?}",
        human.pager
    );
    ensure!(
        matches!(human.width, Some(RenderWidthAttr::Fixed(80))),
        "width: {:?}",
        human.width.as_ref().map(width_name)
    );
    ensure!(
        human.accessibility.as_deref() == Some("text_equivalent"),
        "accessibility: {:?}",
        human.accessibility
    );
    ensure!(
        human.plain.as_deref() == Some("available"),
        "plain: {:?}",
        human.plain
    );
    assert_human_output_leaves(human)
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
    let renderer = parse_renderer(Declaration(&format!("renderer(human(width = {columns}))")))?;
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
    assert_rejected(
        Declaration("renderer(human(width = 0))"),
        Needle("greater than zero"),
    );
}

/// A width above `u16::MAX` is rejected rather than silently truncated.
#[test]
fn rejects_overflowing_width() {
    assert_rejected(
        Declaration("renderer(human(width = 65536))"),
        Needle("must not exceed 65535"),
    );
}

/// An unrecognized width spelling is rejected.
#[test]
fn rejects_unknown_named_width() {
    assert_rejected(
        Declaration(r#"renderer(human(width = "wide"))"#),
        Needle("unknown width 'wide'"),
    );
}

/// A non-string, non-integer width is rejected as the wrong literal type.
#[test]
fn rejects_width_with_wrong_literal_type() {
    assert_rejected(
        Declaration("renderer(human(width = true))"),
        Needle("expected"),
    );
}

/// Unknown top-level renderer keys are rejected.
#[test]
fn rejects_unknown_renderer_key() {
    assert_rejected(
        Declaration(r#"renderer(audio(colour = "auto"))"#),
        Needle("unknown renderer attribute `audio`"),
    );
}

/// Unknown policy keys inside a record are rejected.
#[test]
fn rejects_unknown_record_policy() {
    assert_rejected(
        Declaration(r#"renderer(human(hyperlink = "always"))"#),
        Needle("unknown renderer policy `hyperlink`"),
    );
}

/// Every token leaf rejects a spelling outside its accepted set.
///
/// The rows are deliberately uneven: each names a token list the IR also
/// enforces, so a widened parser list would fail here first.
#[rstest]
#[case::tty(PolicyLeaf(r#"tty = "sometimes""#), Needle("unknown tty 'sometimes'"))]
#[case::closed_stdin(
    PolicyLeaf(r#"closed_stdin = "hangup""#),
    Needle("unknown closed_stdin 'hangup'")
)]
#[case::colour(PolicyLeaf(r#"colour = "on""#), Needle("unknown colour 'on'"))]
#[case::emoji(PolicyLeaf(r#"emoji = "maybe""#), Needle("unknown emoji 'maybe'"))]
#[case::progress(PolicyLeaf(r#"progress = "on""#), Needle("unknown progress 'on'"))]
#[case::pager(PolicyLeaf(r#"pager = "on""#), Needle("unknown pager 'on'"))]
#[case::accessibility(
    PolicyLeaf(r#"accessibility = "full""#),
    Needle("unknown accessibility 'full'")
)]
#[case::plain(PolicyLeaf(r#"plain = "partial""#), Needle("unknown plain 'partial'"))]
#[case::stdout(PolicyLeaf(r#"stdout = "binary""#), Needle("unknown stdout 'binary'"))]
#[case::stderr(PolicyLeaf(r#"stderr = "binary""#), Needle("unknown stderr 'binary'"))]
#[case::progress_inheritance(
    PolicyLeaf(r#"progress_inheritance = "sometimes""#),
    Needle("unknown progress_inheritance 'sometimes'")
)]
#[case::subprocess_inheritance(
    PolicyLeaf(r#"subprocess_inheritance = "sometimes""#),
    Needle("unknown subprocess_inheritance 'sometimes'")
)]
fn rejects_unknown_token(#[case] leaf: PolicyLeaf<'_>, #[case] needle: Needle<'_>) {
    assert_rejected(Declaration(&leaf.in_human_record()), needle);
}

/// A token leaf rejects a non-string literal.
#[test]
fn rejects_token_with_wrong_literal_type() {
    assert_rejected(
        Declaration("renderer(human(colour = 1))"),
        Needle("colour must be a string"),
    );
}

/// `supported` rejects a non-boolean literal.
#[test]
fn rejects_supported_with_wrong_literal_type() {
    assert_rejected(
        Declaration(r#"renderer(human(supported = "yes"))"#),
        Needle("expected"),
    );
}

/// Repeating a leaf, even across groups for the same record, is an error.
#[rstest]
#[case::same_group(Declaration(r#"renderer(human(colour = "auto", colour = "never"))"#))]
#[case::across_groups(Declaration(r#"renderer(human(colour = "auto"), human(colour = "never"))"#))]
fn rejects_duplicate_policy(#[case] decl: Declaration<'_>) {
    assert_rejected(decl, Needle("duplicate renderer policy `colour`"));
}
