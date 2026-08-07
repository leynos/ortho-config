//! Tests for the injectable default project-root resolver.
//!
//! The resolver replaces the ambient `std::env::current_dir()` call in
//! `ConfigDiscoveryBuilder::build`, so these cases inject results rather
//! than changing the process working directory; none of them mutates any
//! process-global state.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rstest::{fixture, rstest};

use crate::MapEnv;
use crate::discovery::ConfigDiscovery;

use super::super::telemetry_test_support::capture_events;

/// A builder isolated from the ambient environment via an empty `MapEnv`.
#[fixture]
fn isolated_builder() -> crate::ConfigDiscoveryBuilder {
    ConfigDiscovery::builder("demo").env_source(Arc::new(MapEnv::new()))
}

/// An injected successful resolver supplies the default project root.
#[rstest]
fn an_injected_resolution_becomes_the_default_project_root(
    isolated_builder: crate::ConfigDiscoveryBuilder,
) {
    let discovery = isolated_builder
        .with_project_root_resolver(Arc::new(|| Ok(PathBuf::from("/injected/root"))))
        .build();

    assert!(
        discovery
            .candidates()
            .iter()
            .any(|path| path == &PathBuf::from("/injected/root/.demo.toml")),
        "the injected root should contribute the project-file candidate"
    );
}

/// An injected failure omits the default root and reports the bounded state.
///
/// The candidate list is pinned exactly: it must equal the list a succeeding
/// resolver produces minus that resolver's root-derived entries, so no
/// fallback root of any kind can slip in unnoticed.
#[rstest]
fn a_failed_resolution_omits_the_root_and_reports_it(
    #[from(isolated_builder)] succeeding_builder: crate::ConfigDiscoveryBuilder,
    #[from(isolated_builder)] failing_builder: crate::ConfigDiscoveryBuilder,
) {
    let succeeding = succeeding_builder
        .with_project_root_resolver(Arc::new(|| Ok(PathBuf::from("/resolved/root"))))
        .build();
    let expected: Vec<PathBuf> = succeeding
        .candidates()
        .iter()
        .filter(|path| !path.starts_with("/resolved/root"))
        .cloned()
        .collect();

    let events = capture_events(|| {
        let discovery = failing_builder
            .with_project_root_resolver(Arc::new(|| {
                Err(io::Error::new(io::ErrorKind::NotFound, "gone"))
            }))
            .build();
        assert_eq!(
            discovery.candidates(),
            expected,
            "a failed resolution must yield exactly the non-root candidates"
        );
    });

    let project_root: Vec<_> = events
        .iter()
        .filter(|event| event.field("event") == "discovery.project_root")
        .collect();
    assert_eq!(project_root.len(), 1, "exactly one project-root event");
    let state = project_root
        .first()
        .map(|event| event.field("state"))
        .unwrap_or_default();
    assert_eq!(state, "cwd_unavailable");
}

/// Explicit project roots suppress the resolver entirely.
#[rstest]
fn explicit_roots_suppress_the_resolver(isolated_builder: crate::ConfigDiscoveryBuilder) {
    let invoked = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&invoked);

    let discovery = isolated_builder
        .add_project_root("/explicit/root")
        .with_project_root_resolver(Arc::new(move || {
            flag.store(true, Ordering::SeqCst);
            Ok(PathBuf::from("/never/used"))
        }))
        .build();

    assert!(
        discovery
            .candidates()
            .iter()
            .any(|path| path == &PathBuf::from("/explicit/root/.demo.toml")),
        "the explicit root should be searched"
    );
    assert!(
        !invoked.load(Ordering::SeqCst),
        "the resolver must not run when explicit roots exist"
    );
}

/// Pin the complete rendering of a representative telemetry event.
///
/// The captured field map is fully injected and path-free, so the exact
/// `Debug` snapshot is stable on every host; any new field must be
/// reviewed here before it can ship, keeping values out of events.
#[rstest]
fn a_failed_resolution_event_renders_exactly(isolated_builder: crate::ConfigDiscoveryBuilder) {
    let events = capture_events(|| {
        let discovery = isolated_builder
            .with_project_root_resolver(Arc::new(|| {
                Err(io::Error::new(io::ErrorKind::NotFound, "gone"))
            }))
            .build();
        drop(discovery.candidates());
    });

    let event = events
        .iter()
        .find(|event| event.field("event") == "discovery.project_root")
        .expect("the project-root event should be captured");
    assert_eq!(
        format!("{event:?}"),
        "CapturedEvent { fields: {\
         \"event\": \"discovery.project_root\", \
         \"message\": \"working directory unavailable; no default project root added\", \
         \"state\": \"cwd_unavailable\"} }"
    );
}
