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
