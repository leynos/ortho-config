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

use anyhow::Context as _;
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{ConfigDiscovery, MapEnv};
use rstest::rstest;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{Layer, Registry};

/// One captured `tracing` event, reduced to its recorded fields.
#[derive(Debug, Default)]
struct Captured {
    fields: BTreeMap<String, String>,
}

impl Captured {
    /// Read a field, or the empty string when the event did not record it.
    fn field(&self, name: &str) -> &str {
        self.fields.get(name).map_or("", String::as_str)
    }
}

#[derive(Default)]
struct Events(Mutex<Vec<Captured>>);

impl Events {
    fn lock(&self) -> MutexGuard<'_, Vec<Captured>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

struct CaptureLayer(Arc<Events>);

impl<S> Layer<S> for CaptureLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _context: Context<'_, S>) {
        let mut captured = Captured::default();
        event.record(&mut FieldVisitor {
            fields: &mut captured.fields,
        });
        self.0.lock().push(captured);
    }
}

struct FieldVisitor<'fields> {
    fields: &'fields mut BTreeMap<String, String>,
}

impl Visit for FieldVisitor<'_> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_owned(), value.to_owned());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_owned(), value.to_string());
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .insert(field.name().to_owned(), format!("{value:?}"));
    }
}

/// Run `f` with a capturing subscriber installed on this thread only.
///
/// `with_default` is thread-local, so these tests need no lock and no
/// `#[serial]` — the same property the injected environment source provides
/// for the environment itself.
fn capture<R>(f: impl FnOnce() -> R) -> Vec<Captured> {
    let events = Arc::new(Events::default());
    let subscriber = Registry::default().with(CaptureLayer(Arc::clone(&events)));
    tracing::subscriber::with_default(subscriber, f);
    std::mem::take(&mut *events.lock())
}

/// Every captured event carrying the given `event` name.
fn named<'a>(events: &'a [Captured], name: &str) -> Vec<&'a Captured> {
    events
        .iter()
        .filter(|event| event.field("event") == name)
        .collect()
}

/// Exactly one event with `name` must have been emitted; return it.
fn only<'a>(events: &'a [Captured], name: &str) -> &'a Captured {
    let matching = named(events, name);
    match matching.as_slice() {
        [event] => event,
        other => panic!("expected exactly one `{name}` event, got {other:?}"),
    }
}

/// Write a minimal loadable configuration file into `dir`, returning its path.
///
/// The write goes through a `cap_std::fs::Dir` handle rather than `std::fs`,
/// which the repository's lint suite requires: a capability handle names the
/// directory it may touch, so a fixture cannot accidentally write relative to
/// the process's working directory.
fn write_fixture(dir: &std::path::Path, name: &str) -> anyhow::Result<std::path::PathBuf> {
    let cap =
        Dir::open_ambient_dir(dir, ambient_authority()).context("open the temporary directory")?;
    cap.write(name, b"value = 1\n")
        .context("write the fixture")?;
    Ok(dir.join(name))
}

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

    let events = capture(|| discovery.candidates());
    assert_eq!(only(&events, "discovery.selector").field("state"), expected);
}

/// The XDG decision separates the two variables and the resolution taken.
///
/// `XDG_CONFIG_DIRS=":"` is the case worth having: the variable is *present*
/// yet every segment is empty, so discovery still falls back to the platform
/// default. Reporting presence and resolution as separate fields is what makes
/// that distinguishable from the variable simply being unset.
#[rstest]
#[case::nothing_set(&[], ("absent", "absent", "default"))]
#[case::config_home_only(&[("XDG_CONFIG_HOME", "/xdg")], ("present", "absent", "default"))]
#[case::empty_config_home(&[("XDG_CONFIG_HOME", "")], ("empty", "absent", "default"))]
#[case::dirs_supply_the_list(&[("XDG_CONFIG_DIRS", "/a")], ("absent", "present", "list"))]
#[case::dirs_present_but_all_empty(&[("XDG_CONFIG_DIRS", ":")], ("absent", "present", "default"))]
#[case::empty_dirs(&[("XDG_CONFIG_DIRS", "")], ("absent", "empty", "default"))]
fn xdg_decision_is_reported(#[case] pairs: &[(&str, &str)], #[case] expected: (&str, &str, &str)) {
    let (expected_config_home, expected_dirs, expected_resolution) = expected;
    let env: MapEnv = pairs.iter().copied().collect();
    let discovery = discovery_with(env);

    let events = capture(|| discovery.candidates());
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

    let events = capture(|| discovery.candidates());
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

    let events = capture(|| discovery.candidates());
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

    let candidate = only(&events, "discovery.candidate");
    assert_eq!(candidate.field("outcome"), "required_failure");
    assert_eq!(candidate.field("candidate_kind"), "required");
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
        for (name, value) in &event.fields {
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
    use std::collections::BTreeMap;

    /// Total each counter across its label sets.
    ///
    /// Summing over labels is deliberate: the assertion is about how many
    /// discovery operations were counted, not about the label vocabulary,
    /// which the tracing tests above already pin.
    fn counter_totals(snapshot: Snapshot) -> BTreeMap<String, u64> {
        let mut totals = BTreeMap::new();
        for (key, _, _, value) in snapshot.into_vec() {
            if let DebugValue::Counter(count) = value {
                *totals.entry(key.key().name().to_owned()).or_insert(0_u64) += count;
            }
        }
        totals
    }

    #[test]
    fn discovery_increments_attempt_and_outcome_counters() {
        let recorder = DebuggingRecorder::new();
        let snapshotter = recorder.snapshotter();

        metrics::with_local_recorder(&recorder, || {
            let discovery = discovery_with(MapEnv::new());
            drop(discovery.load_first());
        });

        let totals = counter_totals(snapshotter.snapshot());
        assert_eq!(
            totals.get("ortho_config.discovery.attempts").copied(),
            Some(1),
            "one discovery attempt should be counted, got {totals:?}"
        );
        assert_eq!(
            totals.get("ortho_config.discovery.outcomes").copied(),
            Some(1),
            "one terminal outcome should be counted, got {totals:?}"
        );
    }
}
