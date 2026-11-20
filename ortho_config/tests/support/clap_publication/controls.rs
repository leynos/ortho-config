//! Durable negative controls for the publication gate.
//!
//! ADR-0028 requires each refusal to be demonstrated against a committed
//! fixture, so these tests are ordinary unit tests over
//! `tests/fixtures/clap_publication/` rather than mutations of the real lock or
//! registry cache. Every control asserts the reason for the failure, not merely
//! that a failure happened: a gate that refused everything would still fail the
//! supportive cases above them.

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Target;
use rstest::rstest;

use super::compare_count;
use super::files::read_text;
use super::identity::{
    REVIEWED_CLAP, REVIEWED_CLAP_BUILDER, verify_lock_edge, verify_lock_identity,
};
use super::library::unique_library_root;
use super::lock::{Lockfile, select_unique};
use super::source::{count_error_kind_variants, count_error_kinds_from, count_error_kinds_under};
use crate::clap_reviewed::CLAP_ERROR_KIND_COUNT;

/// The directory holding the committed gate fixtures.
fn fixture_root() -> Utf8PathBuf {
    Utf8Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/clap_publication")
}

/// Read one fixture as UTF-8 text.
fn read_fixture(relative: &str) -> Result<String> {
    read_text(&fixture_root().join(relative))
}

/// Deserialise one committed `cargo metadata` target list.
fn read_targets(relative: &str) -> Result<Vec<Target>> {
    serde_json::from_str(&read_fixture(relative)?)
        .with_context(|| format!("parse {relative} as cargo metadata targets"))
}

/// The refusal message, or an empty string when the check passed.
fn refusal(result: Result<()>) -> String {
    match result {
        Ok(()) => String::new(),
        Err(error) => error.to_string(),
    }
}

/// Fail unless the refusal names the expected reason.
///
/// These tests return `Result`, so they report with `ensure!` rather than
/// `assert!`: the workspace denies panics inside `Result`-returning functions.
fn ensure_refusal(message: &str, expected: &str) -> Result<()> {
    ensure!(
        message.contains(expected),
        "{message:?} should name {expected:?}",
    );
    Ok(())
}

/// The supported lock satisfies both reviewed identities.
#[test]
fn supported_lock_matches_the_reviewed_identities() -> Result<()> {
    let lock = Lockfile::parse(&read_fixture("supported/Cargo.lock")?)?;
    let facade = select_unique(lock.packages(), REVIEWED_CLAP.name)?;
    let builder = select_unique(lock.packages(), REVIEWED_CLAP_BUILDER.name)?;

    verify_lock_identity(facade, &REVIEWED_CLAP)?;
    verify_lock_identity(builder, &REVIEWED_CLAP_BUILDER)?;
    Ok(())
}

/// The supported lock records exactly one facade edge.
#[test]
fn supported_lock_has_one_facade_edge() -> Result<()> {
    let lock = Lockfile::parse(&read_fixture("supported/Cargo.lock")?)?;
    let facade = select_unique(lock.packages(), REVIEWED_CLAP.name)?;

    verify_lock_edge(facade)
}

/// An unreviewed version fails, and the message names the version.
#[test]
fn unknown_version_fails_the_identity_check() -> Result<()> {
    let lock = Lockfile::parse(&read_fixture("unknown_identity/Cargo.lock")?)?;
    let facade = select_unique(lock.packages(), REVIEWED_CLAP.name)?;

    let message = refusal(verify_lock_identity(facade, &REVIEWED_CLAP));
    ensure_refusal(&message, "reviewed version")
}

/// The reviewed version resolved from another artefact fails on its checksum.
#[test]
fn checksum_drift_fails_the_identity_check() -> Result<()> {
    let lock = Lockfile::parse(&read_fixture("checksum_drift/Cargo.lock")?)?;
    let facade = select_unique(lock.packages(), REVIEWED_CLAP.name)?;

    let message = refusal(verify_lock_identity(facade, &REVIEWED_CLAP));
    ensure_refusal(&message, "reviewed checksum")
}

/// The reviewed version resolved from a replaced source fails on its source.
#[test]
fn source_drift_fails_the_identity_check() -> Result<()> {
    let lock = Lockfile::parse(&read_fixture("source_drift/Cargo.lock")?)?;
    let facade = select_unique(lock.packages(), REVIEWED_CLAP.name)?;

    let message = refusal(verify_lock_identity(facade, &REVIEWED_CLAP));
    ensure_refusal(&message, "reviewed source")
}

/// A lock that lost the builder edge fails, naming the missing dependency.
#[test]
fn missing_facade_edge_fails_the_edge_check() -> Result<()> {
    let lock = Lockfile::parse(&read_fixture("missing_facade_edge/Cargo.lock")?)?;
    let facade = select_unique(lock.packages(), REVIEWED_CLAP.name)?;

    let message = refusal(verify_lock_edge(facade));
    ensure_refusal(&message, "records no clap_builder dependency")
}

/// Two builder versions fail selection rather than choosing one.
#[test]
fn ambiguous_builder_fails_selection() -> Result<()> {
    let lock = Lockfile::parse(&read_fixture("ambiguous_builder/Cargo.lock")?)?;

    let error = refusal(select_unique(lock.packages(), REVIEWED_CLAP_BUILDER.name).map(|_| ()));
    ensure_refusal(&error, "more than one package named")
}

/// The reviewed synthetic source agrees with the reviewed total.
#[test]
fn reviewed_source_agrees_with_the_reviewed_total() -> Result<()> {
    let source = read_fixture("sources/error_kind_reviewed.rs")?;
    let origin = Utf8Path::new("sources/error_kind_reviewed.rs");

    let parsed = count_error_kind_variants(&source, origin)?;
    ensure!(
        parsed == CLAP_ERROR_KIND_COUNT,
        "the reviewed fixture parses {parsed} variants",
    );
    compare_count(parsed, CLAP_ERROR_KIND_COUNT)
}

/// A source with two fewer variants fails the comparison, not the parse.
#[test]
fn drifted_source_fails_the_count_comparison() -> Result<()> {
    let source = read_fixture("sources/error_kind_fifteen.rs")?;
    let origin = Utf8Path::new("sources/error_kind_fifteen.rs");

    let parsed = count_error_kind_variants(&source, origin)?;
    ensure!(
        parsed == 15,
        "the fixture should parse with fifteen variants, found {parsed}",
    );
    let message = refusal(compare_count(parsed, CLAP_ERROR_KIND_COUNT));
    ensure_refusal(&message, "reviewed total is 17")
}

/// A feature-gated variant is refused rather than counted or skipped.
#[test]
fn cfg_gated_variant_is_refused() -> Result<()> {
    let source = read_fixture("sources/error_kind_cfg.rs")?;
    let origin = Utf8Path::new("sources/error_kind_cfg.rs");

    let error = refusal(count_error_kind_variants(&source, origin).map(|_| ()));
    ensure_refusal(&error, "conditionally compiles")
}

/// A truncated enum is refused as a parse error.
#[test]
fn malformed_source_is_refused() -> Result<()> {
    let source = read_fixture("sources/error_kind_malformed.txt")?;
    let origin = Utf8Path::new("sources/error_kind_malformed.txt");

    let error = refusal(count_error_kind_variants(&source, origin).map(|_| ()));
    ensure_refusal(&error, "should be valid Rust")
}

/// Two definitions of the same name are refused as ambiguous.
#[test]
fn duplicate_definitions_are_refused() -> Result<()> {
    let source = read_fixture("sources/two_error_kinds.rs")?;
    let origin = Utf8Path::new("sources/two_error_kinds.rs");

    let error = refusal(count_error_kind_variants(&source, origin).map(|_| ()));
    ensure_refusal(&error, "more than one top-level ErrorKind")
}

/// A file with no definition fails only when it is the expected location.
#[test]
fn file_without_a_definition_is_refused() -> Result<()> {
    let source = read_fixture("sources/no_error_kind.rs")?;
    let origin = Utf8Path::new("sources/no_error_kind.rs");

    let error = refusal(count_error_kind_variants(&source, origin).map(|_| ()));
    ensure_refusal(&error, "declares no top-level ErrorKind")
}

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

/// The crate root comes from the reported targets, not from their shape.
///
/// The fixture also lists a `custom-build` and a `proc-macro` target, so this
/// distinguishes the library filter from "take the last target" as well as from
/// the `src/lib.rs` default.
#[test]
fn reported_library_root_is_selected_from_the_targets() -> Result<()> {
    let targets = read_targets("targets/library_beside_other_targets.json")?;

    let root = unique_library_root(&targets)?;
    ensure!(
        root == Utf8Path::new("lib/entry.rs"),
        "{root} should be the reported library root",
    );
    Ok(())
}

/// A package that reports no library target is refused, not counted.
#[test]
fn package_without_a_library_target_is_refused() -> Result<()> {
    let targets = read_targets("targets/no_library.json")?;

    let error = refusal(unique_library_root(&targets).map(|_| ()));
    ensure_refusal(&error, "no library target")
}

/// Two library targets are refused, whichever library kinds they use.
#[test]
fn two_library_targets_are_refused() -> Result<()> {
    let targets = read_targets("targets/two_libraries.json")?;

    let error = refusal(unique_library_root(&targets).map(|_| ()));
    ensure_refusal(&error, "2 library targets")
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
