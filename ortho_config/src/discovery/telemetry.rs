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

/// Record a single candidate failing to load.
///
/// This is not terminal: discovery continues to the next candidate. `required`
/// distinguishes a failure that will be reported even if a later fallback
/// succeeds from one that is discarded when a fallback works.
pub(super) fn candidate_failure(operation: &'static str, required: bool) {
    let outcome = if required {
        OUTCOME_REQUIRED_FAILURE
    } else {
        OUTCOME_OPTIONAL_FAILURE
    };
    tracing::debug!(
        event = "discovery.candidate",
        operation,
        outcome,
        candidate_kind = if required { "required" } else { "optional" },
        required,
        "configuration candidate rejected"
    );
    count_outcome(operation, outcome);
}

/// Record the terminal outcome of a discovery operation.
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

#[cfg(test)]
mod tests {
    //! Unit tests for the telemetry vocabulary.
    //!
    //! Event emission is covered end to end by `tests/discovery_telemetry.rs`;
    //! these cases pin the classification helper the labels are derived from.

    use super::*;

    #[test]
    fn presence_distinguishes_absent_empty_and_present() {
        assert_eq!(presence(None), PRESENCE_ABSENT);
        assert_eq!(presence(Some(&OsString::new())), PRESENCE_EMPTY);
        assert_eq!(presence(Some(&OsString::from("/xdg"))), PRESENCE_PRESENT);
    }
}
