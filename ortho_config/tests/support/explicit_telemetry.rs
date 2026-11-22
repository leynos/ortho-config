//! Telemetry emitted for explicit selectors and required candidates.
//!
//! These cases share `discovery_telemetry.rs`'s capture harness. They live in
//! a submodule because that suite would otherwise exceed the repository's
//! 400-line file cap, and the neighbouring `load_source` and `policy_telemetry`
//! modules already establish the pattern.

use super::capture_support::{capture, only, write_fixture, write_fixture_with};
use super::discovery_builder::discovery_with;
use ortho_config::{ConfigDiscovery, MapEnv};
use rstest::rstest;
use std::sync::Arc;

/// The selector's four states are distinguished.
///
/// `not_configured` and `unset` look identical from the candidate list — both
/// contribute nothing — yet call for opposite responses: one is a programming
/// decision, the other an operator one. Only the event tells them apart.
#[rstest]
#[case::accepted(Some("DEMO_CONFIG"), Some("/etc/selected.toml"), "accepted")]
#[case::empty(Some("DEMO_CONFIG"), Some(""), "empty")]
#[case::unset(Some("DEMO_CONFIG"), None, "unset")]
#[case::not_configured(None, None, "not_configured")]
fn selector_state_is_reported(
    #[case] env_var: Option<&str>,
    #[case] value: Option<&str>,
    #[case] expected: &str,
) {
    let mut env = MapEnv::new();
    if let Some(selector) = value {
        env.insert("DEMO_CONFIG", selector);
    }
    let mut builder = ConfigDiscovery::builder("demo")
        .clear_project_roots()
        .env_source(Arc::new(env));
    if let Some(name) = env_var {
        builder = builder.env_var(name);
    }
    let discovery = builder.build();

    let events = capture(|| discovery.load_first());
    assert_eq!(
        only(&events, "discovery.selector").field("state"),
        expected,
        "selector state should be {expected}"
    );
}

/// A source that holds one selector and cannot report a working directory.
#[derive(Debug)]
struct NoWorkingDirectory(MapEnv);

impl ortho_config::EnvSource for NoWorkingDirectory {
    fn get(&self, key: &str) -> Option<std::ffi::OsString> {
        self.0.get(key)
    }

    fn current_dir(&self) -> std::io::Result<std::path::PathBuf> {
        Err(std::io::Error::other("deleted"))
    }
}

/// A relative selector that cannot be anchored is reported as dropped, not
/// accepted, while an absolute one is unaffected.
#[rstest]
#[case::relative("chosen.toml", true)]
#[case::absolute("/etc/chosen.toml", false)]
fn selector_without_a_working_directory_is_reported(
    #[case] selector: &str,
    #[case] is_dropped: bool,
) {
    let expected = if is_dropped { "dropped" } else { "accepted" };
    let source = NoWorkingDirectory(MapEnv::new().with_var("DEMO_CONFIG", selector));
    let discovery = ConfigDiscovery::builder("demo")
        .clear_project_roots()
        .env_var("DEMO_CONFIG")
        .env_source(Arc::new(source))
        .build();

    let events = capture(|| discovery.load_first());
    assert_eq!(
        only(&events, "discovery.selector").field("state"),
        expected,
        "selector state for {selector} should be {expected}"
    );
}

/// A missing required candidate is reported as a required failure.
///
/// The terminal outcome is still `not_found`; the per-candidate event is what
/// distinguishes "nothing was there" from "the path you insisted on was not".
#[test]
fn a_missing_required_candidate_reports_a_required_failure() {
    let discovery = ConfigDiscovery::builder("demo")
        .clear_project_roots()
        .add_required_path("/nonexistent/required.toml")
        .env_source(Arc::new(MapEnv::new()))
        .build();

    let events = capture(|| discovery.load_first_partitioned());

    // The doc comment above promises the terminal outcome stays `not_found`;
    // pin it, since that distinction is the whole point of the test.
    assert_eq!(
        only(&events, "discovery.load").field("outcome"),
        "not_found",
        "a missing required candidate should leave the terminal outcome not_found"
    );

    let candidate = only(&events, "discovery.candidate");
    assert_eq!(
        candidate.field("outcome"),
        "required_failure",
        "the candidate event should report a required failure"
    );
    assert_eq!(
        candidate.field("required"),
        "true",
        "the candidate event should mark the failure required"
    );
}

/// A candidate failure names the rung that produced the candidate and why it
/// failed, both drawn from closed vocabularies.
#[test]
fn a_candidate_failure_carries_source_and_category() {
    let discovery = ConfigDiscovery::builder("demo")
        .clear_project_roots()
        .add_required_path("/nonexistent/required.toml")
        .env_source(Arc::new(MapEnv::new()))
        .build();

    let events = capture(|| discovery.load_first_partitioned());

    let candidate = only(&events, "discovery.candidate");
    assert_eq!(
        candidate.field("source"),
        "required_explicit",
        "the candidate source should name the required explicit rung"
    );
    assert_eq!(
        candidate.field("category"),
        "file",
        "the candidate category should be file"
    );
}

/// A selector-supplied candidate that fails to parse is a required failure.
#[test]
fn a_selector_candidate_failure_is_labelled_selector() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let path = write_fixture_with(dir.path(), "broken.toml", "this is not toml = = =\n")
        .expect("fixture should be written");

    // XDG_CONFIG_DIRS pins the base-directory search to the fixture
    // directory, isolating the test from any real /etc/xdg configuration on
    // the host while leaving the selector parse-failure path untouched.
    let discovery = discovery_with(
        MapEnv::new()
            .with_var("DEMO_CONFIG", &path)
            .with_var("XDG_CONFIG_DIRS", dir.path()),
    );
    let events = capture(|| discovery.load_first());

    let candidate = only(&events, "discovery.candidate");
    assert_eq!(
        candidate.field("source"),
        "selector",
        "the candidate source should be selector"
    );
    assert_eq!(
        candidate.field("category"),
        "file",
        "the candidate category should be file"
    );
    assert_eq!(
        candidate.field("required"),
        "true",
        "the selector candidate should be required"
    );
    assert_eq!(
        candidate.field("outcome"),
        "required_failure",
        "the selector candidate should report a required failure"
    );
}

/// A failed selector stays a required failure when a later fallback loads.
///
/// The terminal event reports the winning fallback, so the candidate event is
/// the only place the selector's failure is visible. Its `required` field is
/// what tells an operator the load ultimately failed despite that success.
#[test]
fn a_failed_selector_reports_required_failure_before_a_fallback() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let fallback = write_fixture(dir.path(), "fallback.toml").expect("fixture should be written");
    let missing = dir.path().join("missing.toml");

    let discovery = ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .add_explicit_path(&fallback)
        .clear_project_roots()
        .env_source(Arc::new(MapEnv::new().with_var("DEMO_CONFIG", &missing)))
        .build();
    let events = capture(|| discovery.load_first());

    let candidate = only(&events, "discovery.candidate");
    assert_eq!(
        candidate.field("source"),
        "selector",
        "the failed candidate source should be selector"
    );
    assert_eq!(
        candidate.field("required"),
        "true",
        "the failed selector candidate should be required"
    );
    assert_eq!(
        candidate.field("outcome"),
        "required_failure",
        "the failed selector candidate should report a required failure"
    );

    let load = only(&events, "discovery.load");
    assert_eq!(
        load.field("outcome"),
        "success",
        "the terminal event should report the fallback success"
    );
    assert_eq!(
        load.field("source"),
        "explicit",
        "the terminal event should name the explicit fallback"
    );
}
