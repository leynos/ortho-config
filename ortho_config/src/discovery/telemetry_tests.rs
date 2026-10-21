//! Unit tests for the telemetry vocabulary.
//!
//! Event emission is covered end to end by `tests/discovery_telemetry.rs`;
//! these cases pin the classification helpers the labels are derived from.
//!
//! A sibling of `telemetry` rather than a child module, because the labels it
//! reads and the emission helpers they serve now fill the parent up to the
//! repository's module ceiling.

use std::ffi::OsString;

use super::telemetry::{
    CATEGORY_CYCLIC_EXTENDS, CATEGORY_FILE, CATEGORY_GATHERING, CATEGORY_OTHER,
    CATEGORY_VALIDATION, PRESENCE_ABSENT, PRESENCE_EMPTY, PRESENCE_PRESENT, error_category,
    presence,
};
use super::telemetry_test_support;

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

/// `Captured::field` returns an empty string for a missing field, so
/// absence and emptiness are only distinguishable by the helper's report.
#[test]
fn presence_distinguishes_absent_empty_and_present() {
    assert_eq!(presence(None), PRESENCE_ABSENT);
    assert_eq!(presence(Some(&OsString::new())), PRESENCE_EMPTY);
    assert_eq!(presence(Some(&OsString::from("/xdg"))), PRESENCE_PRESENT);
}

/// A scoped `not_found` load declares `scope` and no `source`.
///
/// `docs/design.md` documents three `discovery.load` spellings with an
/// optional field: `source` only, `scope` only, and both. The integration
/// metadata suite drives the first and the third through real discovery runs;
/// this one has no reachable caller, because the scoped walk emits its
/// per-scope terminal only when a candidate won. Driving the dispatcher
/// directly is therefore the only way to pin the spelling, and doing so keeps
/// a future not-found path from reusing the bare emitter unnoticed.
///
/// The assertion is on metadata, not recorded values: a single emitter that
/// listed both options would record the same values but declare both names,
/// which is exactly the collapse this case exists to catch.
#[test]
fn a_scoped_not_found_load_declares_scope_but_not_source() {
    let events = telemetry_test_support::capture_events(|| {
        super::telemetry::load_outcome(
            super::telemetry::OPERATION_COMPOSE_LAYERS,
            super::telemetry::OUTCOME_NOT_FOUND,
            None,
            Some(super::telemetry::SCOPE_USER),
        );
    });
    let load = events
        .iter()
        .find(|event| event.field("event") == "discovery.load")
        .expect("the dispatcher must emit a discovery.load event");
    assert!(
        load.declares("scope"),
        "a scoped not_found load must declare scope"
    );
    assert!(
        !load.declares("source"),
        "a scoped not_found load must not declare source"
    );
    assert!(load.declares("operation") && load.declares("outcome"));
}
