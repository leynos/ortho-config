//! Telemetry emitted by configuration discovery.
//!
//! Two properties are under test, and the second matters more than the first.
//!
//! The events must be *present and stable*, because an operator diagnosing
//! "why did it load that file?" has nothing else to go on: discovery consults
//! a dozen locations and reports only the winner.
//!
//! The events must also be *empty of secrets*. Everything discovery touches —
//! variable values, resolved paths, file contents — is precisely what must not
//! reach a log. The redaction test below is the enforcement point: it feeds
//! discovery distinctive values and asserts none of them appear in any
//! captured field, so a future field carrying a path fails the suite rather
//! than shipping.

use ortho_config::{ConfigDiscovery, MapEnv};

#[path = "support/tracing_capture.rs"]
mod capture_support;

use capture_support::{capture, only, write_fixture};
use rstest::rstest;
use std::path::Path;
use std::sync::Arc;

fn discovery_with(env: MapEnv) -> ConfigDiscovery {
    ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .clear_project_roots()
        .add_project_root(Path::new("/workspace"))
        .env_source(Arc::new(env))
        .build()
}

#[test]
fn building_with_an_injected_source_is_reported() {
    let events = capture(|| discovery_with(MapEnv::new()));
    assert_eq!(
        only(&events, "discovery.source_selected").field("source"),
        "injected"
    );
}

#[test]
fn building_without_a_source_reports_the_process_environment() {
    let events = capture(|| {
        ConfigDiscovery::builder("demo")
            .clear_project_roots()
            .build()
    });
    assert_eq!(
        only(&events, "discovery.source_selected").field("source"),
        "process"
    );
}

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
    assert_eq!(only(&events, "discovery.selector").field("state"), expected);
}

/// A `XDG_CONFIG_DIRS` value consisting of the separator alone.
///
/// Selected by `cfg!` rather than written as `":"`. The path-list separator is
/// `;` on Windows, so a literal colon is a single *non-empty* segment there:
/// the case would then assert `list` where it means `default`, which is exactly
/// how it failed on the Windows CI leg.
const SEPARATOR_ONLY: &str = if cfg!(windows) { ";" } else { ":" };

/// The XDG decision separates the two variables and the resolution taken.
///
/// A separator-only `XDG_CONFIG_DIRS` is the case worth having: the variable
/// is *present*
/// yet every segment is empty, so discovery still falls back to the platform
/// default. Reporting presence and resolution as separate fields is what makes
/// that distinguishable from the variable simply being unset.
#[rstest]
#[case::nothing_set(&[], ("absent", "absent", "default"))]
#[case::config_home_only(&[("XDG_CONFIG_HOME", "/xdg")], ("present", "absent", "default"))]
#[case::empty_config_home(&[("XDG_CONFIG_HOME", "")], ("empty", "absent", "default"))]
#[case::dirs_supply_the_list(&[("XDG_CONFIG_DIRS", "/a")], ("absent", "present", "list"))]
#[case::dirs_present_but_all_empty(&[("XDG_CONFIG_DIRS", SEPARATOR_ONLY)], ("absent", "present", "default"))]
#[case::empty_dirs(&[("XDG_CONFIG_DIRS", "")], ("absent", "empty", "default"))]
fn xdg_decision_is_reported(#[case] pairs: &[(&str, &str)], #[case] expected: (&str, &str, &str)) {
    let (expected_config_home, expected_dirs, expected_resolution) = expected;
    let env: MapEnv = pairs.iter().copied().collect();
    let discovery = discovery_with(env);

    let events = capture(|| discovery.load_first());
    let event = only(&events, "discovery.xdg");
    assert_eq!(event.field("config_home"), expected_config_home);
    assert_eq!(event.field("dirs"), expected_dirs);
    assert_eq!(event.field("resolution"), expected_resolution);
}

/// Which variable named the home directory is reported, including "none".
#[rstest]
#[case::home(&[("HOME", "/home/injected")], "home")]
#[case::userprofile(&[("USERPROFILE", "/users/injected")], "userprofile")]
#[case::home_outranks_userprofile(
    &[("HOME", "/home/injected"), ("USERPROFILE", "/users/injected")],
    "home"
)]
#[case::neither(&[], "none")]
fn home_decision_is_reported(#[case] pairs: &[(&str, &str)], #[case] expected: &str) {
    let env: MapEnv = pairs.iter().copied().collect();
    let discovery = discovery_with(env);

    let events = capture(|| discovery.load_first());
    assert_eq!(only(&events, "discovery.home").field("source"), expected);
}

/// A source that supplies its own home fallback is reported as `fallback`.
///
/// `MapEnv` cannot exercise this — it models a closed set and returns `None` —
/// so the case needs a bespoke implementor, which is also the point: the
/// fallback is a property of the *source*, not of discovery.
#[test]
fn home_from_the_source_fallback_is_reported() {
    #[derive(Debug)]
    struct FallbackOnlyEnv;

    impl ortho_config::EnvSource for FallbackOnlyEnv {
        fn get(&self, _key: &str) -> Option<std::ffi::OsString> {
            None
        }

        fn home_fallback(&self) -> Option<std::path::PathBuf> {
            Some(std::path::PathBuf::from("/fallback/home"))
        }
    }

    let discovery = ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .clear_project_roots()
        .env_source(Arc::new(FallbackOnlyEnv))
        .build();

    let events = capture(|| discovery.load_first());
    assert_eq!(only(&events, "discovery.home").field("source"), "fallback");
}

#[test]
fn exhausting_every_candidate_reports_not_found() {
    let discovery = discovery_with(MapEnv::new());
    let events = capture(|| discovery.load_first());

    let attempt = only(&events, "discovery.attempt");
    assert_eq!(attempt.field("operation"), "discover_first");

    let load = only(&events, "discovery.load");
    assert_eq!(load.field("operation"), "discover_first");
    assert_eq!(load.field("outcome"), "not_found");
}

#[test]
fn a_loaded_candidate_reports_success() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let path = write_fixture(dir.path(), "selected.toml").expect("fixture should be written");

    let discovery = discovery_with(MapEnv::new().with_var("DEMO_CONFIG", &path));
    let events = capture(|| discovery.load_first());

    let load = only(&events, "discovery.load");
    assert_eq!(load.field("outcome"), "success");
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
        "not_found"
    );

    let candidate = only(&events, "discovery.candidate");
    assert_eq!(candidate.field("outcome"), "required_failure");
    assert_eq!(candidate.field("required"), "true");
}

#[test]
fn compose_layers_reports_its_own_operation() {
    let discovery = discovery_with(MapEnv::new());
    let events = capture(|| discovery.compose_layers());

    assert_eq!(
        only(&events, "discovery.attempt").field("operation"),
        "compose_layers"
    );
    assert_eq!(
        only(&events, "discovery.load").field("operation"),
        "compose_layers"
    );
}

/// No injected value, and no resolved path, may appear in any captured field.
///
/// This is the test that stops telemetry becoming an exfiltration channel.
/// Discovery is handed values that are distinctive enough that an accidental
/// `?path` or `%value` field anywhere in the discovery path fails here, rather
/// than being noticed in production logs.
#[test]
fn no_event_field_carries_an_environment_value_or_path() {
    const SECRETS: &[&str] = &[
        "sekrit-selector",
        "sekrit-xdg-home",
        "sekrit-xdg-dirs",
        "sekrit-appdata",
        "sekrit-localappdata",
        "sekrit-home",
        "sekrit-workspace",
    ];

    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let selected =
        write_fixture(dir.path(), "sekrit-selector.toml").expect("fixture should be written");

    let env = MapEnv::new()
        .with_var("DEMO_CONFIG", &selected)
        .with_var("XDG_CONFIG_HOME", "/sekrit-xdg-home")
        .with_var("XDG_CONFIG_DIRS", "/sekrit-xdg-dirs")
        .with_var("APPDATA", "/sekrit-appdata")
        .with_var("LOCALAPPDATA", "/sekrit-localappdata")
        .with_var("HOME", "/sekrit-home");

    let discovery = ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .clear_project_roots()
        .add_project_root(Path::new("/sekrit-workspace"))
        .env_source(Arc::new(env))
        .build();

    let events = capture(|| {
        drop(discovery.candidates());
        drop(discovery.load_first());
        drop(discovery.compose_layers());
    });

    assert!(
        !events.is_empty(),
        "the redaction check is vacuous unless events were captured"
    );

    for event in &events {
        for (name, value) in event.fields() {
            for secret in SECRETS {
                assert!(
                    !value.contains(secret),
                    "field `{name}` leaked `{secret}`: {value}"
                );
            }
            assert!(
                !value.contains(dir.path().to_string_lossy().as_ref()),
                "field `{name}` leaked the temporary directory: {value}"
            );
        }
    }
}

/// Assembling the candidate list is a query and stays silent.
///
/// The decision events belong to the discovery operations that act on the
/// list; a caller inspecting the search order with `candidates()` performs no
/// discovery and must not generate events claiming otherwise.
#[test]
fn the_candidates_query_emits_no_events() {
    let discovery = discovery_with(MapEnv::new().with_var("HOME", "/home/injected"));
    let events = capture(|| discovery.candidates());
    assert!(
        events.is_empty(),
        "candidates() should be silent, got {} event(s)",
        events.len()
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
    assert_eq!(candidate.field("source"), "required_explicit");
    assert_eq!(candidate.field("category"), "file");
}

/// A selector-supplied candidate that fails to parse is labelled `selector`.
#[test]
fn a_selector_candidate_failure_is_labelled_selector() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let path =
        capture_support::write_fixture_with(dir.path(), "broken.toml", "this is not toml = = =\n")
            .expect("fixture should be written");

    let discovery = discovery_with(MapEnv::new().with_var("DEMO_CONFIG", &path));
    let events = capture(|| discovery.load_first());

    let candidate = only(&events, "discovery.candidate");
    assert_eq!(candidate.field("source"), "selector");
    assert_eq!(candidate.field("category"), "file");
}

#[cfg(feature = "metrics")]
mod metrics_facade {
    //! Counter emission through the `metrics` facade.
    //!
    //! The recorder is installed per-thread via `with_local_recorder`, not
    //! globally: a global recorder can be set only once per process, which
    //! would make these tests order-dependent and mutually exclusive — the
    //! same trap the injected environment source exists to avoid.

    use super::{MapEnv, discovery_with};
    use metrics_util::debugging::{DebugValue, DebuggingRecorder, Snapshot};

    /// Count one counter, restricted to a single label.
    ///
    /// Scoped by label rather than totalled: `count_outcome` fires from both
    /// `candidate_failure` and `load_outcome`, so a bare total would also be
    /// asserting that no candidate failed. That is a host-dependent claim —
    /// `/etc/xdg/demo/config.toml` is a real path that could exist on the
    /// machine running the test — and it is not what this test is about.
    fn counter_for(
        entries: &[(metrics_util::CompositeKey, u64)],
        name: &str,
        label: (&str, &str),
    ) -> u64 {
        entries
            .iter()
            .filter(|(key, _)| {
                key.key().name() == name
                    && key
                        .key()
                        .labels()
                        .any(|found| found.key() == label.0 && found.value() == label.1)
            })
            .map(|(_, count)| *count)
            .sum()
    }

    fn counters(snapshot: Snapshot) -> Vec<(metrics_util::CompositeKey, u64)> {
        snapshot
            .into_vec()
            .into_iter()
            .filter_map(|(key, _, _, value)| match value {
                DebugValue::Counter(count) => Some((key, count)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn discovery_increments_attempt_and_outcome_counters() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();

        metrics::with_local_recorder(&recorder, || {
            let discovery = discovery_with(MapEnv::new());
            drop(discovery.load_first());
        });

        let entries = counters(snapshotter.snapshot());
        assert_eq!(
            counter_for(
                &entries,
                "ortho_config.discovery.attempts",
                ("operation", "discover_first")
            ),
            1,
            "one discovery attempt should be counted, got {entries:?}"
        );
        assert_eq!(
            counter_for(
                &entries,
                "ortho_config.discovery.outcomes",
                ("outcome", "not_found")
            ),
            1,
            "one terminal not_found outcome should be counted, got {entries:?}"
        );
    }
}
