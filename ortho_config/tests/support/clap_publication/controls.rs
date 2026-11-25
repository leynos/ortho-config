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
use cargo_metadata::{NodeDep, Target};

use super::compare_count;
use super::files::read_text;
use super::identity::{
    REVIEWED_ANSTYLE, REVIEWED_CLAP, REVIEWED_CLAP_BUILDER, edge_binds, verify_lock_edge,
    verify_lock_identity, verify_lock_stage,
};
use super::library::unique_library_root;
use super::lock::{Lockfile, select_unique};
use super::source::count_error_kind_variants;
use crate::clap_reviewed::CLAP_ERROR_KIND_COUNT;

/// The directory holding the committed gate fixtures.
pub(super) fn fixture_root() -> Utf8PathBuf {
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
pub(super) fn refusal(result: Result<()>) -> String {
    match result {
        Ok(()) => String::new(),
        Err(error) => error.to_string(),
    }
}

/// Fail unless the refusal names the expected reason.
///
/// These tests return `Result`, so they report with `ensure!` rather than
/// `assert!`: the workspace denies panics inside `Result`-returning functions.
pub(super) fn ensure_refusal(message: &str, expected: &str) -> Result<()> {
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

/// A changed checksum in the lock is refused by the gate's own lock stage.
///
/// This drives [`verify_lock_stage`], the function the gate calls, rather than
/// the identity helper beside it: a lock identical to the reviewed one except
/// for the glob crate's checksum must be refused, and the same lock with the
/// reviewed checksum must pass. Removing the stage's glob check fails this.
#[test]
fn changed_glob_checksum_is_refused_by_the_lock_stage() -> Result<()> {
    let reviewed = read_fixture("lock/reviewed_stage.lock")?;
    let resolved = [
        (REVIEWED_CLAP.name, REVIEWED_CLAP.version),
        (REVIEWED_CLAP_BUILDER.name, REVIEWED_CLAP_BUILDER.version),
        (REVIEWED_ANSTYLE.name, REVIEWED_ANSTYLE.version),
    ];
    verify_lock_stage(&reviewed, &resolved)?;

    let drifted = reviewed.replacen(REVIEWED_ANSTYLE.checksum, &"0".repeat(64), 1);
    ensure!(
        drifted != reviewed,
        "the fixture lock should carry the reviewed glob checksum",
    );
    let error = refusal(verify_lock_stage(&drifted, &resolved));
    ensure_refusal(&error, "reviewed checksum")
}

/// A renamed graph edge is matched by its binding, not its package name.
///
/// Cargo records the import binding in the edge name, so a rename must be
/// reached under that binding while a different crate sharing the package name
/// must not match it. The edges are deserialized because `NodeDep` is
/// non-exhaustive.
#[test]
fn renamed_edge_is_matched_by_its_binding() -> Result<()> {
    let renamed: NodeDep = serde_json::from_str(
        r#"{"name": "anstyle", "pkg": "other 1.0.0 (registry+x)", "dep_kinds": []}"#,
    )?;
    let rebound: NodeDep = serde_json::from_str(
        r#"{"name": "renamed", "pkg": "anstyle 1.0.14 (registry+x)", "dep_kinds": []}"#,
    )?;

    ensure!(
        edge_binds(&renamed, "anstyle"),
        "the renamed edge should bind the requested name",
    );
    ensure!(
        !edge_binds(&rebound, "anstyle"),
        "an edge under another binding must not match the requested name",
    );
    Ok(())
}
