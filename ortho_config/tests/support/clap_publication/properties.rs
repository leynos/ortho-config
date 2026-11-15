//! Property coverage for the `ErrorKind` source inventory.
//!
//! V4 of the `ExecPlan` requires the parser to be exercised as a generator, with
//! each declaration class recorded. The properties here cover four classes:
//!
//! - `declaration_shape_does_not_change_the_count` accepts random declaration
//!   order, doc comments, discriminants, and unit, tuple, and struct variants.
//!   clap's current variants are unit-only, but the counter must not filter a
//!   shape it might see later.
//! - `a_cfg_variant_fails_closed` rejects both `cfg` and `cfg_attr`, so a
//!   feature-dependent inventory is reported instead of silently counted.
//! - `a_truncated_declaration_fails` rejects malformed input, so a parse error
//!   can never be mistaken for a complete inventory.
//!
//! Every strategy emits ASCII only, so byte offsets used by the truncation
//! property are also character boundaries.

use std::fmt;
use std::fmt::Write as _;

use camino::Utf8Path;
use proptest::prelude::*;

use super::source::count_error_kind_variants;

/// The declaration shape of one generated variant.
#[derive(Clone, Copy, Debug)]
enum Shape {
    /// `Variant`.
    Unit,
    /// `Variant(u8)`.
    Tuple,
    /// `Variant { code: u8 }`.
    Struct,
}

/// One generated `ErrorKind` variant.
#[derive(Clone, Debug)]
struct VariantSpec {
    /// Variant identifier.
    name: String,
    /// Whether the variant carries a doc comment.
    documented: bool,
    /// Explicit discriminant; renderable only on unit variants.
    discriminant: Option<u16>,
    /// Declaration shape.
    shape: Shape,
}

/// Strategy producing one plausible variant declaration.
fn variant_spec() -> impl Strategy<Value = VariantSpec> {
    (
        "[A-Z][A-Za-z0-9]{0,7}",
        any::<bool>(),
        prop::option::of(0u16..=u16::MAX),
        prop::sample::select(vec![Shape::Unit, Shape::Tuple, Shape::Struct]),
    )
        .prop_map(|(name, documented, discriminant, shape)| VariantSpec {
            name,
            documented,
            discriminant,
            shape,
        })
}

/// Render a synthetic `ErrorKind` whose variants are the generated specs.
///
/// `gate` optionally inserts one attribute before the chosen variant, which is
/// how the conditional-declaration property builds its input.
fn render_enum(
    variants: &[VariantSpec],
    gate: Option<(usize, &str)>,
) -> std::result::Result<String, fmt::Error> {
    let mut source = String::from("pub enum ErrorKind {\n");
    for (index, variant) in variants.iter().enumerate() {
        write_gate(&mut source, gate, index)?;
        if variant.documented {
            writeln!(source, "    /// Generated variant")?;
        }
        write_variant(&mut source, variant)?;
    }
    source.push_str("}\n");
    Ok(source)
}

/// Writes the gated attribute when `index` is the variant it annotates.
fn write_gate(
    source: &mut String,
    gate: Option<(usize, &str)>,
    index: usize,
) -> std::result::Result<(), fmt::Error> {
    match gate {
        Some((gated, attribute)) if gated == index => writeln!(source, "    {attribute}"),
        _ => Ok(()),
    }
}

/// Writes the declaration line for one generated variant.
fn write_variant(
    source: &mut String,
    variant: &VariantSpec,
) -> std::result::Result<(), fmt::Error> {
    match variant.shape {
        Shape::Unit => write_unit_variant(source, variant),
        Shape::Tuple => writeln!(source, "    {}(u8),", variant.name),
        Shape::Struct => writeln!(source, "    {} {{ code: u8 }},", variant.name),
    }
}

/// Writes a unit variant, with its discriminant when it has one.
fn write_unit_variant(
    source: &mut String,
    variant: &VariantSpec,
) -> std::result::Result<(), fmt::Error> {
    match variant.discriminant {
        Some(value) => writeln!(source, "    {} = {value},", variant.name),
        None => writeln!(source, "    {},", variant.name),
    }
}

/// The origin named in refusals raised by these properties.
const ORIGIN: &str = "generated/error_kind.rs";

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    /// Declaration order, comments, discriminants, and shapes preserve the count.
    #[test]
    fn declaration_shape_does_not_change_the_count(
        variants in prop::collection::vec(variant_spec(), 0..24),
    ) {
        let source = render_enum(&variants, None).expect("the generated enum should render");
        let count = count_error_kind_variants(&source, Utf8Path::new(ORIGIN))
            .expect("a well-formed declaration should parse");
        prop_assert_eq!(count, variants.len());
    }

    /// A `cfg` or `cfg_attr` variant fails closed rather than being counted.
    #[test]
    fn a_cfg_variant_fails_closed(
        variants in prop::collection::vec(variant_spec(), 1..12),
        index in any::<prop::sample::Index>(),
        attribute in prop::sample::select(vec![
            "#[cfg(feature = \"generated\")]",
            "#[cfg_attr(test, allow(dead_code))]",
        ]),
    ) {
        let gated = index.index(variants.len());
        let source = render_enum(&variants, Some((gated, attribute)))
            .expect("the generated enum should render");
        prop_assert!(
            count_error_kind_variants(&source, Utf8Path::new(ORIGIN)).is_err(),
            "a conditional declaration must be refused",
        );
    }

    /// Any truncated declaration fails instead of counting a partial inventory.
    #[test]
    fn a_truncated_declaration_fails(
        variants in prop::collection::vec(variant_spec(), 0..12),
        index in any::<prop::sample::Index>(),
    ) {
        let source = render_enum(&variants, None).expect("the generated enum should render");
        let cut = index.index(source.len() - 1);
        let truncated = source
            .get(..cut)
            .expect("the generated source is ASCII, so every offset is a boundary");
        prop_assert!(
            count_error_kind_variants(truncated, Utf8Path::new(ORIGIN)).is_err(),
            "a truncated declaration must be refused",
        );
    }
}
