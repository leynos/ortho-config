//! Fixture publications that drive the production artefact boundary.
//!
//! `lock_tests`, `publication_tests`, and `report_tests` all need to run a real
//! `publish::publish` against a temporary root, differing only in the identity
//! they publish under and the observation configuration they pass. Those
//! builders live here so each suite drives the production code rather than
//! re-implementing the sequence — which is how the earlier lock tests came to
//! assert against `lock` directly while never observing `publish` at all.
//!
//! The temporary root itself is *not* duplicated here: `TempRoot` is owned by
//! `fragment_tests`, so every helper below takes a root path instead.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use proc_macro2::Span;
use syn::Ident;

use super::LocalizationIds;
use super::artefact_report::ReportConfig;
use super::publish;
use super::schema::Source;
use crate::derive::generate::localization::{CommandIds, FluentMessageId, LocalizationBase};

/// Runs a full fragment publication so the lock wraps real emission work.
///
/// This drives the production `publish` boundary rather than re-implementing
/// the sequence, so the exclusivity and release assertions observe exactly the
/// code the compiler runs.
pub(super) fn publish(root: &Path) -> Result<()> {
    publish_as(root, "fixture", "fixture")
}

/// Publishes one fixture fragment under an explicit observation configuration.
///
/// The observation tests need the production boundary with the channel turned
/// on, which the helper modes cannot express: their configuration travels
/// through the process environment, and a test that set the opt-in variable
/// would change it for every test running concurrently in this binary.
pub(super) fn publish_fixture(root: &Path, report: ReportConfig) -> Result<()> {
    publish_configured(root, "fixture", "fixture", report)
}

/// Publishes one fragment under a crate-specific identity.
///
/// Each identity writes a distinct fragment filename and records a distinct
/// crate name, so the merged inventory shows whether a later publication
/// retained an earlier publication's entries.
pub(super) fn publish_as(root: &Path, label: &str, crate_name: &str) -> Result<()> {
    publish_configured(root, label, crate_name, ReportConfig::quiet())
}

/// Publishes one named variant of the fixture source.
pub(super) fn publish_variant(root: &Path, variant: &str) -> Result<()> {
    publish_as(root, variant, variant)
}

/// Publishes one fragment with an explicit observation configuration.
fn publish_configured(
    root: &Path,
    label: &str,
    crate_name: &str,
    report: ReportConfig,
) -> Result<()> {
    let ident = Ident::new("Config", Span::call_site());
    // The fragment filename mixes the source path and the line, so keying the
    // path by label keeps every identity's fragment distinct. The file has to
    // exist: `merge_fragments` treats a vanished source as a stale fragment
    // and drops its entries, which would silently empty this fixture.
    let source = Source {
        file: fixture_source_path(root, label)?,
        line: 1,
        column: 0,
    };
    let model = fixture_model();
    publish::publish(
        &publish::Publication {
            root,
            ident: &ident,
            source: &source,
            crate_name,
            model: &model,
            report,
        },
        Span::call_site(),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}

/// Creates a stand-in source file and returns its absolute path.
///
/// The path must be absolute because the merge probes it from whatever working
/// directory the test binary happens to run in.
pub(super) fn fixture_source_path(root: &Path, label: &str) -> Result<String> {
    let path = root.join(format!("{label}-source.rs"));
    fs::write(&path, "// fixture source\n").context("create fixture source file")?;
    Ok(path.to_string_lossy().into_owned())
}

/// Builds the command-id group a fixture publication records.
///
/// The ids are never resolved against a catalogue; publication records them
/// verbatim, so any stable set exercises the same write path.
fn fixture_model() -> LocalizationIds {
    let id = |name: &str| FluentMessageId(name.to_owned());
    LocalizationIds {
        base: LocalizationBase("fixture".to_owned()),
        command: CommandIds {
            about_id: id("fixture"),
            long_about_id: id("fixture"),
            usage_id: id("fixture"),
            version_id: id("fixture"),
            long_version_id: id("fixture"),
            after_help_id: id("fixture"),
            after_long_help_id: id("fixture"),
        },
        args: Vec::new(),
    }
}
