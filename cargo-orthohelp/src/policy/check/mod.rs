//! Orchestration for the `--check-agent-native` policy check.
//!
//! Implements Decisions D5, D6, D11 and D13: evaluate the package's policy
//! configuration (no bridge build), write `policy-report.json` atomically,
//! print a human summary to standard error, and return a `PolicyViolation`
//! error when deny-mode findings are present. The package is resolved by the
//! caller using the light `metadata` selection, so this module stays free of
//! the generator's `cli`/`metadata` dependencies.

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Package;
use std::io::Write;

use crate::error::OrthohelpError;
use crate::output;
use crate::policy::config::{PolicyConfig, PolicyConfigMetadata, PolicyInputs};
use crate::policy::evaluate::evaluate;
use crate::policy::{PolicyMode, PolicyReport};

/// Outcome of a policy check run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyCheckOutcome {
    /// Path of the written policy report.
    pub report_path: Utf8PathBuf,
    /// Enforcement mode used by the run.
    pub mode: PolicyMode,
}

/// Runs the agent-native policy check for one package.
///
/// Reads the optional policy table from the package's Cargo metadata, applies
/// the `--policy-mode` override to the *report* mode, evaluates, writes the
/// report atomically (Decision D5), and prints the human summary before any
/// write (so it appears even when the artefact write fails). In `deny` mode
/// with at least one deny finding, a [`OrthohelpError::PolicyViolation`] is
/// returned after the report has been written (Decision D6).
///
/// # Errors
///
/// Returns a metadata error when the policy table is structurally invalid in
/// any mode, an I/O error when the report cannot be written, and a
/// `PolicyViolation` under deny mode with deny findings.
pub fn run_policy_check(
    package: &Package,
    mode_override: Option<PolicyMode>,
    out_dir: &Utf8Path,
) -> Result<PolicyCheckOutcome, OrthohelpError> {
    let table_found = policy_table_found(package);
    let metadata_config = PolicyConfigMetadata::from_package_metadata(&package.metadata)
        .map_err(OrthohelpError::MetadataJson)?;
    let mut config = PolicyConfig::from(metadata_config.unwrap_or_default());
    if let Some(override_mode) = mode_override {
        config.mode = override_mode;
    }
    let report = evaluate(&config, &PolicyInputs::default());
    let report_path = out_dir.join("policy-report.json");
    write_summary(&report, &report_path, table_found, &package.name)?;
    output::write_policy_report(out_dir, &report)?;
    if config.mode == PolicyMode::Deny && report.summary.deny > 0 {
        return Err(OrthohelpError::PolicyViolation {
            deny_count: report.summary.deny,
            report_path: report_path.to_string(),
        });
    }
    Ok(PolicyCheckOutcome {
        report_path,
        mode: config.mode,
    })
}

/// Reports whether the package declares an `ortho_config.policy` table.
fn policy_table_found(package: &Package) -> bool {
    policy_table_in_metadata(&package.metadata)
}

/// Reports whether a `package.metadata` value declares a policy table.
fn policy_table_in_metadata(metadata: &serde_json::Value) -> bool {
    metadata
        .get("ortho_config")
        .is_some_and(|value| value.get("policy").is_some())
}

/// Prints the short human summary to standard error.
fn write_summary(
    report: &PolicyReport,
    report_path: &Utf8Path,
    table_found: bool,
    package_name: &str,
) -> Result<(), OrthohelpError> {
    writeln!(
        std::io::stderr().lock(),
        "{}",
        summary_line(report, report_path, table_found, package_name),
    )
    .map_err(|io_err| OrthohelpError::Io {
        path: report_path.to_path_buf(),
        source: io_err,
    })
}

/// Renders the one-line human summary for the check.
///
/// The off-mode wording is deliberately loud (Decision D13) so a check that
/// configured nothing does not read as a passing gate.
fn summary_line(
    report: &PolicyReport,
    report_path: &Utf8Path,
    table_found: bool,
    package_name: &str,
) -> String {
    if report.mode == PolicyMode::Off {
        let reason = if table_found {
            "configured or overridden to off"
        } else {
            "no [package.metadata.ortho_config.policy] table found"
        };
        format!("policy mode off ({reason}); nothing was checked; report: {report_path}")
    } else {
        format!(
            "policy check for {package_name} complete; mode: {}; findings: {} warn, {} deny; report: {report_path}",
            report.mode, report.summary.warn, report.summary.deny,
        )
    }
}

#[cfg(test)]
mod tests;
