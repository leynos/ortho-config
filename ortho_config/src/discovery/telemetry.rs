//! Structured telemetry for configuration discovery.
//!
//! Discovery's inputs and outputs are exactly the values that must never reach
//! a log: environment variable values, resolved filesystem paths, and the
//! contents of the files it opens. Every event here therefore records a
//! *decision* rather than the datum the decision was made from.
//!
//! That is enforced by construction: each field is a `&'static str` drawn from
//! a closed set declared in this module, or a `bool`. No caller can pass a path
//! or a variable value through these functions, and the same property keeps the
//! metric labels bounded — a cardinality bug here would otherwise show up as an
//! unbounded time-series in a consumer's monitoring system.
//!
//! Metrics are emitted through the [`metrics`] facade behind the optional
//! `metrics` feature. The facade is inert unless the consuming binary installs
//! a recorder, so the default build pays nothing and the library never chooses
//! a metrics backend on an application's behalf.

use std::ffi::OsString;

/// Discovery consulted the live process environment.
pub(super) const SOURCE_PROCESS: &str = "process";
/// Discovery consulted an injected [`crate::EnvSource`].
pub(super) const SOURCE_INJECTED: &str = "injected";

/// No selector variable was configured on the builder.
pub(super) const SELECTOR_NOT_CONFIGURED: &str = "not_configured";
/// A selector variable was configured but is unset in the environment.
pub(super) const SELECTOR_UNSET: &str = "unset";
/// The selector variable is set to an empty value and is therefore ignored.
pub(super) const SELECTOR_EMPTY: &str = "empty";
/// The selector named a path, which now leads the candidate list.
pub(super) const SELECTOR_ACCEPTED: &str = "accepted";

/// The variable is unset.
pub(super) const PRESENCE_ABSENT: &str = "absent";
/// The variable is set but empty, so it contributes nothing.
pub(super) const PRESENCE_EMPTY: &str = "empty";
/// The variable is set to a usable value.
pub(super) const PRESENCE_PRESENT: &str = "present";

/// The built-in `/etc/xdg` fallback supplied the directory list.
pub(super) const XDG_RESOLUTION_DEFAULT: &str = "default";
/// `XDG_CONFIG_DIRS` supplied the directory list.
pub(super) const XDG_RESOLUTION_LIST: &str = "list";

/// `HOME` named the home directory.
pub(super) const HOME_FROM_HOME: &str = "home";
/// `USERPROFILE` named the home directory.
pub(super) const HOME_FROM_USERPROFILE: &str = "userprofile";
/// The source's platform fallback named the home directory.
pub(super) const HOME_FROM_FALLBACK: &str = "fallback";
/// No home directory could be determined, so no home candidates were added.
pub(super) const HOME_NONE: &str = "none";

/// A path the builder was told is required.
pub(super) const CANDIDATE_REQUIRED_EXPLICIT: &str = "required_explicit";
/// A path the builder was given explicitly but optionally.
pub(super) const CANDIDATE_EXPLICIT: &str = "explicit";
/// The configuration-path selector variable named the path.
pub(super) const CANDIDATE_SELECTOR: &str = "selector";
/// An XDG base directory (explicit or the platform default) supplied the path.
pub(super) const CANDIDATE_XDG: &str = "xdg";
/// A Windows application-data directory supplied the path.
pub(super) const CANDIDATE_WINDOWS: &str = "windows";
/// The home directory (its `.config` tree or the dotfile) supplied the path.
pub(super) const CANDIDATE_HOME: &str = "home";
/// A project root supplied the path.
pub(super) const CANDIDATE_PROJECT: &str = "project";

/// The candidate's file could not be read or was required but absent.
pub(super) const CATEGORY_FILE: &str = "file";
/// The candidate's `extends` chain loops back on itself.
pub(super) const CATEGORY_CYCLIC_EXTENDS: &str = "cyclic_extends";
/// The candidate parsed but gathering its figures failed.
pub(super) const CATEGORY_GATHERING: &str = "gathering";
/// The candidate loaded but a value failed validation.
pub(super) const CATEGORY_VALIDATION: &str = "validation";
/// Any other error the closed set above does not name.
pub(super) const CATEGORY_OTHER: &str = "other";

/// The `load_first`/`compose_layer` family.
pub(super) const OPERATION_DISCOVER_FIRST: &str = "discover_first";
/// The `compose_layers` extends-chain family.
pub(super) const OPERATION_COMPOSE_LAYERS: &str = "compose_layers";

/// A candidate loaded and the operation returned it.
pub(super) const OUTCOME_SUCCESS: &str = "success";
/// Every candidate was exhausted without finding a configuration file.
pub(super) const OUTCOME_NOT_FOUND: &str = "not_found";
/// An optional candidate failed; discovery continued past it.
pub(super) const OUTCOME_OPTIONAL_FAILURE: &str = "optional_failure";
/// A required candidate failed; the error is reported regardless of fallbacks.
pub(super) const OUTCOME_REQUIRED_FAILURE: &str = "required_failure";

/// Classify a variable as absent, empty, or present.
///
/// Discovery treats an empty value as contributing nothing — joining it with an
/// application name yields a working-directory-relative path — so the three
/// states are behaviourally distinct and each deserves its own label.
pub(super) fn presence(value: Option<&OsString>) -> &'static str {
    match value {
        None => PRESENCE_ABSENT,
        Some(inner) if inner.is_empty() => PRESENCE_EMPTY,
        Some(_) => PRESENCE_PRESENT,
    }
}

/// Record which environment source a [`crate::ConfigDiscovery`] was built with.
pub(super) fn source_selected(source: &'static str) {
    tracing::debug!(
        event = "discovery.source_selected",
        source,
        "configuration discovery environment source selected"
    );
}

/// Record how the configuration-path selector resolved.
pub(super) fn selector_decision(state: &'static str) {
    tracing::debug!(
        event = "discovery.selector",
        state,
        "configuration path selector resolved"
    );
}

/// Record the XDG base-directory decision.
pub(super) fn xdg_decision(
    config_home: &'static str,
    dirs: &'static str,
    resolution: &'static str,
) {
    tracing::debug!(
        event = "discovery.xdg",
        config_home,
        dirs,
        resolution,
        "XDG base directories resolved"
    );
}

/// Record which variable, if any, supplied the home directory.
pub(super) fn home_decision(source: &'static str) {
    tracing::debug!(
        event = "discovery.home",
        source,
        "home directory for discovery resolved"
    );
}

/// Record that a discovery operation has begun.
pub(super) fn attempt(operation: &'static str) {
    tracing::debug!(
        event = "discovery.attempt",
        operation,
        "configuration discovery started"
    );
    count_attempt(operation);
}

/// Classify an error into the closed `CATEGORY_*` vocabulary.
///
/// The raw error carries paths and values, so only its variant reaches an
/// event; the category answers "why did this candidate fail" without leaking
/// what it was.
pub(super) const fn error_category(err: &crate::OrthoError) -> &'static str {
    match err {
        crate::OrthoError::File { .. } => CATEGORY_FILE,
        crate::OrthoError::CyclicExtends { .. } => CATEGORY_CYCLIC_EXTENDS,
        crate::OrthoError::Gathering(_) => CATEGORY_GATHERING,
        crate::OrthoError::Validation { .. } => CATEGORY_VALIDATION,
        _ => CATEGORY_OTHER,
    }
}

/// Record a single candidate failing to load.
///
/// This is not terminal: discovery continues to the next candidate. `required`
/// distinguishes a failure that will be reported even if a later fallback
/// succeeds from one that is discarded when a fallback works. `outcome` and
/// `required` deliberately encode the same bit — no third rendering is added —
/// while `source` and `category` carry the two facts the pair cannot: which
/// rung produced the candidate and why it failed, each drawn from a closed set.
pub(super) fn candidate_failure(
    operation: &'static str,
    required: bool,
    source: &'static str,
    category: &'static str,
) {
    let outcome = if required {
        OUTCOME_REQUIRED_FAILURE
    } else {
        OUTCOME_OPTIONAL_FAILURE
    };
    tracing::debug!(
        event = "discovery.candidate",
        operation,
        outcome,
        required,
        source,
        category,
        "configuration candidate rejected"
    );
    count_outcome(operation, outcome);
    count_candidate_failure(operation, source, category);
}

/// Record that no working directory was available for the project-root
/// fallback, so no default project root was added.
pub(super) fn project_root_cwd_unavailable() {
    tracing::debug!(
        event = "discovery.project_root",
        state = "cwd_unavailable",
        "working directory unavailable; no default project root added"
    );
}

/// Record a discovery operation succeeding, naming the rung that won.
///
/// Success is the one terminal outcome with a winning candidate, and "which
/// location did this configuration come from?" is the question an operator
/// actually asks. `source` answers it from the closed `CANDIDATE_*` vocabulary,
/// so the event names the rung without naming the file.
///
/// The metric is deliberately left alone: `count_outcome` keeps its existing
/// `operation`/`outcome` label pair. `source` would multiply that series by the
/// number of rungs to record a fact the event already carries, and the label
/// set is part of the contract a consumer's dashboards are built against.
pub(super) fn load_success(operation: &'static str, source: &'static str) {
    tracing::debug!(
        event = "discovery.load",
        operation,
        outcome = OUTCOME_SUCCESS,
        source,
        "configuration discovery finished"
    );
    count_outcome(operation, OUTCOME_SUCCESS);
}

/// Record a terminal outcome that has no winning candidate.
///
/// Use [`load_success`] when a candidate was returned; this reports the
/// outcomes — `not_found` and its kin — where no rung won and so no `source`
/// field would be meaningful.
pub(super) fn load_outcome(operation: &'static str, outcome: &'static str) {
    tracing::debug!(
        event = "discovery.load",
        operation,
        outcome,
        "configuration discovery finished"
    );
    count_outcome(operation, outcome);
}

#[cfg(feature = "metrics")]
fn count_attempt(operation: &'static str) {
    metrics::counter!("ortho_config.discovery.attempts", "operation" => operation).increment(1);
}

#[cfg(not(feature = "metrics"))]
const fn count_attempt(_operation: &'static str) {}

#[cfg(feature = "metrics")]
fn count_outcome(operation: &'static str, outcome: &'static str) {
    metrics::counter!(
        "ortho_config.discovery.outcomes",
        "operation" => operation,
        "outcome" => outcome,
    )
    .increment(1);
}

#[cfg(not(feature = "metrics"))]
const fn count_outcome(_operation: &'static str, _outcome: &'static str) {}

#[cfg(feature = "metrics")]
fn count_candidate_failure(operation: &'static str, source: &'static str, category: &'static str) {
    metrics::counter!(
        "ortho_config.discovery.candidate_failures",
        "operation" => operation,
        "source" => source,
        "category" => category,
    )
    .increment(1);
}

#[cfg(not(feature = "metrics"))]
const fn count_candidate_failure(
    _operation: &'static str,
    _source: &'static str,
    _category: &'static str,
) {
}

#[cfg(test)]
mod tests {
    //! Unit tests for the telemetry vocabulary.
    //!
    //! Event emission is covered end to end by `tests/discovery_telemetry.rs`;
    //! these cases pin the classification helper the labels are derived from.

    use super::*;

    /// Every `OrthoError` variant maps to its closed-set category.
    ///
    /// The integration suites drive only the `file` category, so a wrong arm
    /// for the others would go unnoticed without this table: each variant is
    /// constructed and checked against the label consumers key dashboards on.
    #[test]
    fn error_category_covers_every_variant() {
        use crate::OrthoError;

        let file = OrthoError::File {
            path: std::path::PathBuf::from("demo.toml"),
            source: Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "gone")),
        };
        assert_eq!(error_category(&file), CATEGORY_FILE);

        let cyclic = OrthoError::CyclicExtends {
            cycle: String::from("a -> b -> a"),
        };
        assert_eq!(error_category(&cyclic), CATEGORY_CYCLIC_EXTENDS);

        let gathering = OrthoError::Gathering(Box::new(figment::Error::from(String::from(
            "gathering failed",
        ))));
        assert_eq!(error_category(&gathering), CATEGORY_GATHERING);

        let validation = OrthoError::Validation {
            key: String::from("port"),
            message: String::from("out of range"),
        };
        assert_eq!(error_category(&validation), CATEGORY_VALIDATION);

        let other = OrthoError::CliParsing(Box::new(clap::Error::raw(
            clap::error::ErrorKind::InvalidValue,
            "bad flag",
        )));
        assert_eq!(error_category(&other), CATEGORY_OTHER);
    }

    #[test]
    fn presence_distinguishes_absent_empty_and_present() {
        assert_eq!(presence(None), PRESENCE_ABSENT);
        assert_eq!(presence(Some(&OsString::new())), PRESENCE_EMPTY);
        assert_eq!(presence(Some(&OsString::from("/xdg"))), PRESENCE_PRESENT);
    }
}
