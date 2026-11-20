//! Module-tree controls for the publication gate.
//!
//! The source walk follows the module files the compiler compiles with the
//! reported crate root, and refuses every shape it cannot bind to the exported
//! inventory. Each fixture here is a committed tree, so the controls run as
//! ordinary unit tests rather than mutations of the real registry source. The
//! accepted trees show the walk is not a blanket refusal; each refused tree
//! names the distinction it exercises, so a gate that refused every tree, or
//! refused one for another shape's reason, still fails against this table.

use anyhow::{Result, ensure};
use camino::Utf8Path;
use rstest::rstest;

use super::controls::{ensure_refusal, fixture_root, refusal};
use super::source::{count_error_kinds_from, count_error_kinds_under};

/// The tree whose module chain the compiler always follows is counted.
///
/// It reaches `ErrorKind` through a plain `mod` file, a `mod.rs`, and a nested
/// declaration, so both resolution shapes are exercised.
#[test]
fn unconditional_module_chain_is_counted() -> Result<()> {
    let root = fixture_root().join("module_tree/unconditional");

    let count = count_error_kinds_under(&root)?;
    ensure!(
        count == 3,
        "the unconditional fixture should count three variants, found {count}",
    );
    Ok(())
}

/// Module-tree shapes the gate must refuse, each for its own stated reason.
///
/// Every case is a committed fixture and the expected text names the
/// distinction that shape exercises, so a gate that refused every tree, or
/// refused one for another shape's reason, still fails here.
#[rstest]
// A file module behind a `#[cfg]` hop is not compiled unconditionally.
#[case::conditional_hop("conditional_hop", "does not unconditionally compile it")]
// A declaration no module declaration reaches is neither compiled nor counted.
#[case::orphan("orphan", "does not unconditionally compile it")]
// A module file whose own inner attributes gate its whole contents.
#[case::file_gated("file_gated", "conditionally compiles the whole file")]
// The compiler expands `#[cfg_attr(all(), cfg_attr(all(), cfg(any())))]`, so a
// walk that inspects only immediate attributes would certify an excluded file.
#[case::cfg_attr_nested("cfg_attr_nested", "does not unconditionally compile it")]
// A `#[path]` makes the compiler load a file a default-name walk never sees.
#[case::path_override("path_override", "carries a `#[path]` attribute")]
// A compiled decoy beside an inline inventory declares exactly the reviewed
// total, so a file-level scan would pass on the wrong enum.
#[case::nested_definition("nested_definition", "inside `mod error`")]
// `#[cfg(all())]` always holds, so the nested inventory is compiled and the
// decoy beside it must not be counted in its place.
#[case::conditional_nested_definition("conditional_nested_definition", "inside `mod error`")]
// A condition applied through a nested `cfg_attr` does not exempt it either.
#[case::cfg_attr_nested_definition("cfg_attr_nested_definition", "inside `mod error`")]
// `include!` splices a contained file the walk never reads, so the merged
// inventory is invisible and the decoy beside it must not be counted.
#[case::macro_generated_definition(
    "macro_generated_definition",
    "its expansion can declare items this walk cannot read"
)]
// A local wrapper whose visible transcription spells no `ErrorKind` but calls
// `include!` hides the same splice one level down, so the unresolved nested
// name must be refused rather than trusted.
#[case::macro_wrapped_include(
    "macro_wrapped_include",
    "its expansion can declare items this walk cannot read"
)]
// A local `assert_eq!` shadows the prelude macro the inert list trusts, so the
// name must be resolved to the local transcription before it is believed inert.
#[case::shadowed_inert_macro(
    "shadowed_inert_macro",
    "its expansion can declare items this walk cannot read"
)]
// The same shadowing applies at item level: a local `compile_error!` is not the
// diagnostic-only macro, so a name-only exemption would accept its splice.
#[case::shadowed_item_macro(
    "shadowed_item_macro",
    "its expansion can declare items this walk cannot read"
)]
// A later redefinition does not reach an earlier call, so the name must be
// resolved against the definitions in scope at that call, not the whole list.
#[case::redefined_macro(
    "redefined_macro",
    "its expansion can declare items this walk cannot read"
)]
// An item-position parameter pastes its argument into the expansion, so the
// macro names in the arguments are a second route the audit has to resolve.
#[case::macro_argument_include(
    "macro_argument_include",
    "its expansion can declare items this walk cannot read"
)]
// A `use` item binds an imported macro to `compile_error`, so the inert name is
// not the prelude macro: the import has to be resolved before it is trusted.
#[case::imported_inert_macro(
    "imported_inert_macro",
    "its expansion can declare items this walk cannot read"
)]
// The same import route reaches a nested inert name, so a transcription that
// invokes `assert_eq!` may be calling an imported splice rather than the
// assertion the inert list trusts.
#[case::imported_nested_macro(
    "imported_nested_macro",
    "its expansion can declare items this walk cannot read"
)]
// `#[macro_use]` imports a plain `macro_rules!` definition into the enclosing
// module without any `use` item at the call, so the attribute itself has to be
// resolved before an inert name is trusted in the modules below it.
#[case::macro_use_inert_macro(
    "macro_use_inert_macro",
    "its expansion can declare items this walk cannot read"
)]
// `#[cfg_attr(all(), macro_use)]` is expanded by the compiler into
// `#[macro_use]`, so a check of the immediate attribute path would miss an
// import that really happens and trust the splices it brings in.
#[case::cfg_attr_macro_use(
    "cfg_attr_macro_use",
    "its expansion can declare items this walk cannot read"
)]
// `r#include!` is the same macro as `include!` and the rendering keeps the
// prefix, so an unrecognized spelling must not read as naming no macro at all.
#[case::raw_identifier_include(
    "raw_identifier_include",
    "its expansion can declare items this walk cannot read"
)]
// A tree without a reported crate root is refused rather than guessed at.
#[case::no_crate_root("no_crate_root", "expected a crate root lib.rs")]
fn module_tree_is_refused(#[case] fixture: &str, #[case] expected: &str) -> Result<()> {
    let root = fixture_root().join("module_tree").join(fixture);

    let error = refusal(count_error_kinds_under(&root).map(|_| ()));
    ensure_refusal(&error, expected)
}

/// An unconditional inline module is followed rather than assumed opaque.
#[test]
fn inline_module_hop_is_followed() -> Result<()> {
    let root = fixture_root().join("module_tree/inline_hop");

    let count = count_error_kinds_under(&root)?;
    ensure!(
        count == 2,
        "the inline-module fixture should count two variants, found {count}",
    );
    Ok(())
}

/// An unrelated import does not make an inert name ambiguous.
///
/// The import audit refuses an inert name any `use` item in the module could
/// have bound. A blanket refusal of every module carrying imports would fail
/// this fixture, which imports another name and still uses `compile_error!` as
/// the diagnostic-only macro the inert list describes.
#[test]
fn unrelated_imports_leave_inert_names_trusted() -> Result<()> {
    let root = fixture_root().join("module_tree/imports_beside_inert_macro");

    let count = count_error_kinds_under(&root)?;
    ensure!(
        count == 3,
        "the accepted-import fixture should count three variants, found {count}",
    );
    Ok(())
}

/// The count follows the reported root, not the default one beside it.
///
/// `[lib] path` moves the crate root to `lib/entry.rs`, whose `mod kind;` sits
/// beside it. The unused `src/lib.rs` declares seventeen variants, so a walk
/// that started at the default location would report a total for source the
/// build never compiles.
#[test]
fn count_follows_the_reported_library_root() -> Result<()> {
    let root = fixture_root().join("module_tree/custom_library_root");

    let counted = count_error_kinds_from(&root.join("lib"), Utf8Path::new("entry.rs"))?;
    ensure!(
        counted == 3,
        "the reported root should count three variants, found {counted}",
    );
    let decoy = count_error_kinds_from(&root.join("src"), Utf8Path::new("lib.rs"))?;
    ensure!(
        decoy == 17,
        "the unused default root should count seventeen variants, found {decoy}",
    );
    Ok(())
}
