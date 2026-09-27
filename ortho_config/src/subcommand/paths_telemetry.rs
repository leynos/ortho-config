//! Bounded structured telemetry for subcommand file discovery.
//!
//! Discovery decides which base directories to consult, whether a named
//! variable or a platform fallback supplied each one, and which candidate
//! files exist. All of that is driven by paths, environment values, and host
//! state — none of which are safe event fields. This module therefore accepts
//! no caller-controlled text: every emitted field is drawn from the closed
//! vocabularies below.
//!
//! Failures are reported with the closed category only. The affected path and
//! the underlying I/O error reach the caller through the returned
//! [`OrthoError`], never through an event field.
//!
//! When the optional `metrics` feature is enabled, the same closed vocabulary
//! labels discovery counters. The library installs no recorder, so embedding
//! applications retain ownership of metric export.

use crate::{OrthoError, OrthoResult};

/// The discovery stage that resolved the ordered base directories.
const STAGE_BASES: &str = "bases";
/// The discovery stage that probed candidate files.
const STAGE_CANDIDATES: &str = "candidates";

/// An absolute named value supplied the Unix configuration home.
const SOURCE_NAMED: &str = "named";
/// The home fallback supplied the Unix configuration home.
const SOURCE_FALLBACK: &str = "fallback";
/// Neither a named value nor a fallback supplied a configuration home.
const SOURCE_ABSENT: &str = "absent";
/// `USERPROFILE` supplied the non-Unix home.
#[cfg(not(any(unix, target_os = "redox")))]
const SOURCE_USERPROFILE: &str = "userprofile";
/// A platform lookup supplied the non-Unix configuration directory.
#[cfg(not(any(unix, target_os = "redox")))]
const SOURCE_PLATFORM_FALLBACK: &str = "platform_fallback";

/// Discovery produced its result.
const OUTCOME_SUCCESS: &str = "success";
/// Discovery could not produce its result.
const OUTCOME_FAILURE: &str = "failure";
/// A candidate was selected because the path exists.
const OUTCOME_EXISTS: &str = "exists";
/// A candidate was absent, so the search moved to the next base.
const OUTCOME_ABSENT: &str = "absent";
/// Probing a candidate failed for a reason other than absence.
const OUTCOME_PROBE_FAILED: &str = "probe_failed";

/// No error applies to a decision or a successful discovery.
const CATEGORY_NONE: &str = "none";
/// A candidate probe failed for a reason other than absence.
const CATEGORY_PROBE: &str = "probe";
/// A candidate file could not be loaded.
const CATEGORY_FILE: &str = "file";
/// An environment layer could not be gathered.
const CATEGORY_GATHERING: &str = "gathering";
/// Layer extraction or CLI merging failed.
const CATEGORY_MERGE: &str = "merge";
/// A loaded value failed validation.
const CATEGORY_VALIDATION: &str = "validation";
/// Command-line parsing prevented a complete load.
const CATEGORY_CLI: &str = "cli";
/// An inferred clap default could not be converted.
const CATEGORY_DEFAULT_VALUE_CONVERSION: &str = "default_value_conversion";
/// A configuration `extends` cycle prevented loading.
const CATEGORY_CYCLIC_EXTENDS: &str = "cyclic_extends";
/// Several loading errors were retained for reporting together.
const CATEGORY_AGGREGATE: &str = "aggregate";

/// Record that an absolute named value supplied the Unix configuration home.
pub(super) fn unix_config_home_from_named() {
    base_event(SOURCE_NAMED);
}

/// Record that the home fallback supplied the Unix configuration home.
pub(super) fn unix_config_home_from_fallback() {
    base_event(SOURCE_FALLBACK);
}

/// Record that no configuration home was available, so none was added.
pub(super) fn unix_config_home_absent() {
    base_event(SOURCE_ABSENT);
}

/// Record that `USERPROFILE` supplied the non-Unix home.
#[cfg(not(any(unix, target_os = "redox")))]
pub(super) fn windows_home_from_userprofile() {
    base_event(SOURCE_USERPROFILE);
}

/// Record that a platform lookup supplied the non-Unix configuration directory.
#[cfg(not(any(unix, target_os = "redox")))]
pub(super) fn windows_config_dir_from_platform() {
    base_event(SOURCE_PLATFORM_FALLBACK);
}

/// Record that a home variable supplied the non-Unix home.
#[cfg(not(any(unix, target_os = "redox")))]
pub(super) fn windows_home_from_named() {
    base_event(SOURCE_NAMED);
}

/// Record that no home was available for the non-Unix candidate list.
#[cfg(not(any(unix, target_os = "redox")))]
pub(super) fn windows_home_absent() {
    base_event(SOURCE_ABSENT);
}

/// Record that no non-Unix configuration directory was available.
#[cfg(not(any(unix, target_os = "redox")))]
pub(super) fn windows_config_dir_absent() {
    base_event(SOURCE_ABSENT);
}

/// Emit one base-resolution decision from the closed source vocabulary.
///
/// The stage and outcome carry no variable content; `source` names which kind
/// of lookup won, not the value it returned.
fn base_event(source: &'static str) {
    tracing::debug!(
        event = "subcommand.paths.bases",
        stage = STAGE_BASES,
        outcome = OUTCOME_SUCCESS,
        source,
        category = CATEGORY_NONE,
        "subcommand discovery base resolved"
    );
    count_base(source);
}

/// Record the start of candidate gathering for one subcommand.
pub(super) fn candidates_started() {
    tracing::debug!(
        event = "subcommand.paths.candidates",
        stage = STAGE_CANDIDATES,
        outcome = OUTCOME_SUCCESS,
        category = CATEGORY_NONE,
        "subcommand configuration candidate search started"
    );
    count_candidates(OUTCOME_SUCCESS, CATEGORY_NONE);
}

/// Record the terminal outcome of candidate gathering.
///
/// Only the category is recorded; the error's path and source stay in the
/// returned value.
pub(super) fn candidates_finished<T>(result: &OrthoResult<T>) {
    let (outcome, category) = match result {
        Ok(_) => (OUTCOME_SUCCESS, CATEGORY_NONE),
        Err(error) => (OUTCOME_FAILURE, error_category(error)),
    };
    tracing::debug!(
        event = "subcommand.paths.candidates",
        stage = STAGE_CANDIDATES,
        outcome,
        category,
        "subcommand configuration candidate search finished"
    );
    count_candidates(outcome, category);
}

/// Record that a candidate was selected because its path exists.
///
/// Only the base's position in the resolved list is recorded, which is enough
/// for an operator to tell a shadowed candidate from a missing one without
/// disclosing any path. Keeping the position as a separate event means the
/// event's field set stays fixed at the macro call.
pub(super) fn candidate_exists(position: usize) {
    candidate_position_event(position, OUTCOME_EXISTS, CATEGORY_NONE);
    count_candidate(OUTCOME_EXISTS);
}

/// Record that no base held the candidate, so none was selected.
pub(super) fn candidate_absent() {
    candidate_event(OUTCOME_ABSENT, CATEGORY_NONE);
    count_candidate(OUTCOME_ABSENT);
}

/// Record a probe that failed for a reason other than absence.
pub(super) fn candidate_probe_failed() {
    candidate_event(OUTCOME_PROBE_FAILED, CATEGORY_PROBE);
    count_candidate(OUTCOME_PROBE_FAILED);
}

/// Emit one candidate event from the closed outcome and category vocabularies.
fn candidate_event(outcome: &'static str, category: &'static str) {
    tracing::debug!(
        event = "subcommand.paths.candidate",
        stage = STAGE_CANDIDATES,
        outcome,
        category,
        "subcommand configuration candidate probed"
    );
}

/// Emit the selected-candidate event, naming its base position but no path.
fn candidate_position_event(position: usize, outcome: &'static str, category: &'static str) {
    let position = u32::try_from(position).unwrap_or(u32::MAX);
    tracing::debug!(
        event = "subcommand.paths.candidate",
        stage = STAGE_CANDIDATES,
        position,
        outcome,
        category,
        "subcommand configuration candidate selected"
    );
}

/// Reduce a configuration error to the discovery telemetry's closed vocabulary.
const fn error_category(error: &OrthoError) -> &'static str {
    match error {
        OrthoError::CliParsing(_) => CATEGORY_CLI,
        OrthoError::DefaultValueConversion { .. } => CATEGORY_DEFAULT_VALUE_CONVERSION,
        OrthoError::File { .. } => CATEGORY_FILE,
        OrthoError::CyclicExtends { .. } => CATEGORY_CYCLIC_EXTENDS,
        OrthoError::Gathering(_) => CATEGORY_GATHERING,
        OrthoError::Merge { .. } => CATEGORY_MERGE,
        OrthoError::Validation { .. } => CATEGORY_VALIDATION,
        OrthoError::Aggregate(_) => CATEGORY_AGGREGATE,
    }
}

/// Increment the optional counter for a resolved base directory.
#[cfg(feature = "metrics")]
fn count_base(source: &'static str) {
    metrics::counter!(
        "ortho_config.subcommand.paths.bases",
        "source" => source,
    )
    .increment(1);
}

#[cfg(not(feature = "metrics"))]
const fn count_base(_source: &'static str) {}

/// Increment the optional counter for a started or finished candidate search.
#[cfg(feature = "metrics")]
fn count_candidates(outcome: &'static str, category: &'static str) {
    metrics::counter!(
        "ortho_config.subcommand.paths.candidates",
        "outcome" => outcome,
        "category" => category,
    )
    .increment(1);
}

#[cfg(not(feature = "metrics"))]
const fn count_candidates(_outcome: &'static str, _category: &'static str) {}

/// Increment the optional counter for one probed candidate.
#[cfg(feature = "metrics")]
fn count_candidate(outcome: &'static str) {
    metrics::counter!(
        "ortho_config.subcommand.paths.candidate",
        "outcome" => outcome,
    )
    .increment(1);
}

#[cfg(not(feature = "metrics"))]
const fn count_candidate(_outcome: &'static str) {}
