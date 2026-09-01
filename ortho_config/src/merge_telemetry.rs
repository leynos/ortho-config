//! Bounded structured telemetry for environment-layer merging.
//!
//! Merge operations handle environment keys and values, file-derived layers,
//! and deserialisation errors. None of those data are safe event fields. This
//! module therefore accepts no caller-controlled text: every emitted field is
//! selected from one of the closed vocabularies below.

use crate::{OrthoError, OrthoResult};

/// The merge provider delegates enumeration to the live process.
const SOURCE_PROCESS: &str = "process";
/// The merge provider scans a caller-supplied source.
const SOURCE_INJECTED: &str = "injected";

/// A direct `CsvEnv` provider operation.
const OPERATION_CSV_ENV: &str = "csv_env";
/// A derive-generated complete configuration load.
const OPERATION_DERIVED_LOAD: &str = "derived_load";
/// A source-aware subcommand defaults-and-CLI load.
const OPERATION_SUBCOMMAND_LOAD: &str = "subcommand_load";

/// The operation has selected its environment source and begun work.
const OUTCOME_ATTEMPT: &str = "attempt";
/// The operation produced a configuration layer or configuration value.
const OUTCOME_SUCCESS: &str = "success";
/// The operation returned an error after selecting its environment source.
const OUTCOME_FAILURE: &str = "failure";

/// No error applies to an attempt or successful operation.
const CATEGORY_NONE: &str = "none";
/// A `map` or `filter_map` closure could not be replayed under injection.
const CATEGORY_OPAQUE_KEY_TRANSFORM: &str = "opaque_key_transform";
/// A transformed environment key could not form an object layer.
const CATEGORY_INVALID_NESTING: &str = "invalid_nesting";
/// Command-line parsing prevented a complete source-aware load.
const CATEGORY_CLI: &str = "cli";
/// Configuration-file gathering prevented a complete source-aware load.
const CATEGORY_FILE: &str = "file";
/// A configuration `extends` cycle prevented loading.
const CATEGORY_CYCLIC_EXTENDS: &str = "cyclic_extends";
/// A Figment provider could not gather a configuration layer.
const CATEGORY_GATHERING: &str = "gathering";
/// Layer extraction or CLI merging failed.
const CATEGORY_MERGE: &str = "merge";
/// A loaded value did not meet the configuration's validation rules.
const CATEGORY_VALIDATION: &str = "validation";
/// Several loading errors were retained for reporting together.
const CATEGORY_AGGREGATE: &str = "aggregate";

/// Record the decision to gather an environment layer from the live process.
pub(super) fn csv_env_process_started() {
    attempt(OPERATION_CSV_ENV, SOURCE_PROCESS);
}

/// Record the decision to gather an environment layer from an injected source.
pub(super) fn csv_env_injected_started() {
    attempt(OPERATION_CSV_ENV, SOURCE_INJECTED);
}

/// Record a successful process-backed CSV environment merge.
pub(super) fn csv_env_process_succeeded() {
    success(OPERATION_CSV_ENV, SOURCE_PROCESS);
}

/// Record a successful injected CSV environment merge.
pub(super) fn csv_env_injected_succeeded() {
    success(OPERATION_CSV_ENV, SOURCE_INJECTED);
}

/// Record a CSV environment merge failure without exposing provider data.
pub(super) fn csv_env_failed(is_injected: bool, is_opaque_transform: bool) {
    let source = if is_injected {
        SOURCE_INJECTED
    } else {
        SOURCE_PROCESS
    };
    let category = if is_opaque_transform {
        CATEGORY_OPAQUE_KEY_TRANSFORM
    } else {
        CATEGORY_INVALID_NESTING
    };
    failure(OPERATION_CSV_ENV, source, category);
}

/// Record the start of a generated load that has both injected sources.
pub(crate) fn source_aware_derived_load_started() {
    attempt(OPERATION_DERIVED_LOAD, SOURCE_INJECTED);
}

/// Record the terminal outcome of a generated load without serialising errors.
pub(crate) fn source_aware_derived_load_finished<T>(result: &OrthoResult<T>) {
    result_outcome(OPERATION_DERIVED_LOAD, result);
}

/// Record the start of a source-aware subcommand load.
pub(super) fn source_aware_subcommand_load_started() {
    attempt(OPERATION_SUBCOMMAND_LOAD, SOURCE_INJECTED);
}

/// Record the terminal outcome of a source-aware subcommand load.
pub(super) fn source_aware_subcommand_load_finished<T>(result: &OrthoResult<T>) {
    result_outcome(OPERATION_SUBCOMMAND_LOAD, result);
}

/// Emit a decision event using only closed operation, source, and outcome sets.
fn attempt(operation: &'static str, source: &'static str) {
    tracing::debug!(
        event = "merge.layer",
        operation,
        source,
        outcome = OUTCOME_ATTEMPT,
        category = CATEGORY_NONE,
        "environment merge started"
    );
}

/// Emit a successful terminal event using the fixed no-error category.
fn success(operation: &'static str, source: &'static str) {
    tracing::debug!(
        event = "merge.layer",
        operation,
        source,
        outcome = OUTCOME_SUCCESS,
        category = CATEGORY_NONE,
        "environment merge finished"
    );
}

/// Emit a failed terminal event after reducing an error to a closed category.
fn failure(operation: &'static str, source: &'static str, category: &'static str) {
    tracing::debug!(
        event = "merge.layer",
        operation,
        source,
        outcome = OUTCOME_FAILURE,
        category,
        "environment merge failed"
    );
}

/// Record a result while ensuring error contents never become event fields.
fn result_outcome<T>(operation: &'static str, result: &OrthoResult<T>) {
    match result {
        Ok(_) => success(operation, SOURCE_INJECTED),
        Err(error) => failure(operation, SOURCE_INJECTED, error_category(error)),
    }
}

/// Reduce a configuration error to the merge telemetry's closed vocabulary.
const fn error_category(error: &OrthoError) -> &'static str {
    match error {
        OrthoError::CliParsing(_) => CATEGORY_CLI,
        OrthoError::File { .. } => CATEGORY_FILE,
        OrthoError::CyclicExtends { .. } => CATEGORY_CYCLIC_EXTENDS,
        OrthoError::Gathering(_) => CATEGORY_GATHERING,
        OrthoError::Merge { .. } => CATEGORY_MERGE,
        OrthoError::Validation { .. } => CATEGORY_VALIDATION,
        OrthoError::Aggregate(_) => CATEGORY_AGGREGATE,
    }
}
