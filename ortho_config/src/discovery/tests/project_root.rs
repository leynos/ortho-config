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

use crate::MapEnv;
use crate::discovery::ConfigDiscovery;

use super::super::telemetry_test_support::capture_events;

fn isolated_builder() -> crate::ConfigDiscoveryBuilder {
    ConfigDiscovery::builder("demo").env_source(Arc::new(MapEnv::new()))
}

/// An injected successful resolver supplies the default project root.
#[test]
fn an_injected_resolution_becomes_the_default_project_root() {
    let discovery = isolated_builder()
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
#[test]
fn a_failed_resolution_omits_the_root_and_reports_it() {
    let events = capture_events(|| {
        let discovery = isolated_builder()
            .with_project_root_resolver(Arc::new(|| {
                Err(io::Error::new(io::ErrorKind::NotFound, "gone"))
            }))
            .build();
        assert!(
            !discovery
                .candidates()
                .iter()
                .any(|path| path.ends_with(".demo.toml") && !path.starts_with("/")),
            "no default project root should be added"
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
#[test]
fn explicit_roots_suppress_the_resolver() {
    let invoked = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&invoked);

    let discovery = isolated_builder()
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
