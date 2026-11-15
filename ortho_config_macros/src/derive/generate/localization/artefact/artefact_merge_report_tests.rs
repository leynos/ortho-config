//! The merge failure's category, asserted through the production boundary.
//!
//! `report_failure_tests` proves the channel can record a category it is
//! *handed*: it writes a report whose failure it built itself. What that
//! cannot show is whether a failing publication *produces* the right one. The
//! two merge failures this crate can report are classified where the failure
//! is known rather than by the step that spans them, so a caller that guessed
//! a single category for the whole merge would satisfy the other suite while
//! mislabelling every malformed fragment.
//!
//! Both cases are provoked through filesystem topology, so the failure
//! reaching the report is one a real build could hit, and the category is read
//! from the report's own `error` field. It is deliberately not inferred from
//! the diagnostic text: that text is free-form, and an assertion over it would
//! silently change meaning with a message edit.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::artefact_report::{ReportConfig, Reporting, report_path};
use super::fragment_tests::TempRoot;
use super::lock_support::publish_fixture;

/// Returns the report the opted-in publication in this process wrote.
fn published_report(root: &Path) -> Result<Value> {
    let raw = fs::read_to_string(report_path(root)).context("read the build report")?;
    serde_json::from_str(&raw).context("the report must be whole JSON")
}

/// Asserts a report names the merge stage and the expected failure category.
///
/// The stage is asserted alongside the category because the category alone
/// would be satisfied by a failure anywhere in the sequence; `merge` is what
/// makes the value evidence about the merge rather than about the run.
fn ensure_failed_at_merge(report: &Value, category: &str) -> Result<()> {
    let field = |name: &str| report.get(name).and_then(Value::as_str);
    ensure!(
        field("stage") == Some("merge"),
        "a merge failure must report the merge stage; got {report}",
    );
    ensure!(
        field("outcome") == Some("failed"),
        "a failed merge must report a failed outcome; got {report}",
    );
    ensure!(
        field("error") == Some(category),
        "the report must classify this merge failure as {category}; got {report}",
    );
    Ok(())
}

/// Verifies a fragment the filesystem refuses to read reports `io`.
///
/// A directory where a fragment file belongs is the portable provocation: the
/// merge enumerates it, tests its `.json` extension, and is refused when it
/// reads it. The extension is what makes this reach the read rather than the
/// enumeration, so the failure under test is the one the report classifies.
#[test]
fn a_merge_read_failure_reports_io() -> Result<()> {
    let root = TempRoot::new()?;
    let unreadable = root
        .path()
        .join(super::FRAGMENT_DIR)
        .join("unreadable.json");
    fs::create_dir_all(&unreadable).context("place a directory where a fragment must be read")?;

    publish_fixture(root.path(), ReportConfig::observed(Reporting::On))
        .expect_err("a fragment the filesystem refuses to read must fail the publication");

    ensure_failed_at_merge(&published_report(root.path())?, "io")?;
    Ok(())
}

/// Verifies a fragment that is not JSON reports `serialization`.
///
/// This is the case a single-category merge would mislabel: the step is the
/// same merge, but the fault is a file this version cannot parse rather than a
/// filesystem refusal, and the two call for different responses from whoever
/// reads the report.
#[test]
fn a_merge_parse_failure_reports_serialization() -> Result<()> {
    let root = TempRoot::new()?;
    let fragments = root.path().join(super::FRAGMENT_DIR);
    fs::create_dir_all(&fragments).context("create the fragment directory")?;
    fs::write(fragments.join("malformed.json"), "not json")
        .context("write a fragment that is not JSON")?;

    publish_fixture(root.path(), ReportConfig::observed(Reporting::On))
        .expect_err("a fragment that is not JSON must fail the publication");

    ensure_failed_at_merge(&published_report(root.path())?, "serialization")?;
    Ok(())
}
