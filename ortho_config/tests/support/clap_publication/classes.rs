//! Recorded acceptance and classification for every declaration class.
//!
//! V4 of the `ExecPlan` requires the inventory parser to be exercised as a
//! generator and to record generator acceptance and classification, so every
//! relevant case is known to have been exercised. `properties` supplies the
//! generators; this table supplies the record. Each class below is a named,
//! deterministic input paired with the classification the gate must produce for
//! it, so the outcome is observed on every run instead of being inferred from
//! whichever draw a random case happened to make.
//!
//! The classes are the shapes the supported source can plausibly present, plus
//! every shape the gate must refuse. Variant order, doc comments, explicit
//! discriminants, and the three variant shapes must not change the count; an
//! empty enum counts zero; a conditional variant, a nested declaration whether
//! or not its module is conditional, a malformed source, a duplicate
//! declaration, and a file with no declaration must each be refused for the
//! reason recorded beside it.

use anyhow::{Result, ensure};
use camino::Utf8Path;
use rstest::rstest;

use super::source::count_error_kind_variants;

/// What the gate must do with one declaration class.
#[derive(Clone, Copy, Debug)]
enum Classification {
    /// Count this many variants.
    Counted(usize),
    /// Refuse, and name this reason.
    Refused(&'static str),
}

/// The origin named in these classifications.
const ORIGIN: &str = "classes/error_kind.rs";

#[rstest]
#[case::unit_variants(
    "pub enum ErrorKind {\n    Alpha,\n    Beta,\n}\n",
    Classification::Counted(2)
)]
#[case::tuple_variants(
    "pub enum ErrorKind {\n    Alpha(u8),\n    Beta(String),\n}\n",
    Classification::Counted(2)
)]
#[case::struct_variants(
    "pub enum ErrorKind {\n    Alpha { code: u8 },\n    Beta { code: u16 },\n}\n",
    Classification::Counted(2)
)]
#[case::doc_comments(
    "pub enum ErrorKind {\n    /// First.\n    Alpha,\n    /// Second.\n    Beta,\n}\n",
    Classification::Counted(2)
)]
#[case::explicit_discriminants(
    "pub enum ErrorKind {\n    Alpha = 7,\n    Beta = 11,\n}\n",
    Classification::Counted(2)
)]
#[case::reordered_declarations(
    "pub enum ErrorKind {\n    Beta,\n    Alpha,\n}\n",
    Classification::Counted(2)
)]
#[case::empty_enum("pub enum ErrorKind {\n}\n", Classification::Counted(0))]
#[case::cfg_variant(
    "pub enum ErrorKind {\n    Alpha,\n    #[cfg(feature = \"generated\")]\n    Beta,\n}\n",
    Classification::Refused("conditionally compiles")
)]
#[case::cfg_attr_variant(
    "pub enum ErrorKind {\n    Alpha,\n    #[cfg_attr(test, allow(dead_code))]\n    Beta,\n}\n",
    Classification::Refused("conditionally compiles")
)]
#[case::nested_declaration(
    "pub mod error {\n    pub enum ErrorKind {\n        Alpha,\n    }\n}\n",
    Classification::Refused("inside `mod error`")
)]
#[case::conditional_nested_declaration(
    "#[cfg(all())]\npub mod error {\n    pub enum ErrorKind {\n        Alpha,\n    }\n}\n",
    Classification::Refused("inside `mod error`")
)]
#[case::nested_cfg_attr_declaration(
    "#[cfg_attr(all(), cfg(all()))]\npub mod error {\n    pub enum ErrorKind {\n        Alpha,\n    }\n}\n",
    Classification::Refused("inside `mod error`")
)]
#[case::malformed_source(
    "pub enum ErrorKind {\n    Alpha,\n",
    Classification::Refused("should be valid Rust")
)]
#[case::duplicate_declarations(
    "pub enum ErrorKind {\n    Alpha,\n}\n\npub enum ErrorKind {\n    Beta,\n}\n",
    Classification::Refused("more than one top-level ErrorKind")
)]
#[case::no_declaration(
    "pub struct NotTheInventory;\n",
    Classification::Refused("declares no top-level ErrorKind")
)]
fn every_declaration_class_is_classified(
    #[case] source: &str,
    #[case] expected: Classification,
) -> Result<()> {
    let observed = count_error_kind_variants(source, Utf8Path::new(ORIGIN));
    match expected {
        Classification::Counted(count) => {
            let counted = observed?;
            ensure!(
                counted == count,
                "the class should count {count} variants, counted {counted}",
            );
        }
        Classification::Refused(reason) => {
            let message = observed
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default();
            ensure!(
                message.contains(reason),
                "{message:?} should name {reason:?}",
            );
        }
    }
    Ok(())
}
