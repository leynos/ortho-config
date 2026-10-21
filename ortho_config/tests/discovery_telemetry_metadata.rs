//! The discovery events' *metadata* contract, not just their recorded values.
//!
//! `docs/design.md` says an absent optional field is absent from the event,
//! and `tracing` makes that a stronger statement than it first appears: a
//! field's name lives in the callsite's static metadata, so listing an
//! `Option` in one macro call declares the name for every emission. Recording
//! `None` hides only the value.
//!
//! The value-level suites cannot see this. They read a captured field and get
//! the empty string for "absent", which is exactly what a suppressed `None`
//! also yields — so a regression that collapsed the conditional calls back
//! into one macro would leave every one of them green. These cases read
//! `Metadata::fields` instead, which is the layer a subscriber's filtering
//! uses and the layer the documentation is written about.

use ortho_config::{ConfigDiscovery, ConfigFilePolicy, ConfigPathSelector, MapEnv};

#[path = "support/metadata_fields.rs"]
mod metadata_support;

use metadata_support::{all_metadata, capture_metadata, make_dir, only_metadata, write_fixture};
use std::path::PathBuf;
use std::sync::Arc;

/// A policy over a discovery that reads only `env`.
fn policy_with(env: MapEnv) -> ConfigFilePolicy {
    ConfigFilePolicy::from_builder(
        ConfigDiscovery::builder("demo")
            .clear_project_roots()
            .env_source(Arc::new(env)),
    )
}

/// Run the user-scope walk over a fixture carrying `content`.
///
/// Both scoped cases need the same one-scope layout: a `XDG_CONFIG_HOME`
/// candidate with a fixed name, and a `HOME` that yields nothing so the
/// candidate under test is the only one that can fire. The stylesheet is the
/// only difference between them, so the setup lives here once.
///
/// Returns `Result` rather than panicking: this is an ordinary helper, not a
/// `#[test]` fn, so `allow-expect-in-tests` does not reach it. Each caller
/// unwraps it deliberately, where the lint does apply, so a setup failure
/// fails the case that needed the fixture with a message naming that case.
fn run_user_scope(content: &str) -> anyhow::Result<Vec<metadata_support::CapturedMetadata>> {
    use ortho_config::{AutomaticMode, DiscoveryScope};

    let temp = tempfile::tempdir()?;
    let app_dir = make_dir(temp.path(), "xdg-home/demo")?;
    write_fixture(&app_dir, "config.toml", content)?;
    let empty_home = make_dir(temp.path(), "home")?;

    let discovery = ConfigDiscovery::builder("demo")
        .config_file_name("config.toml")
        .clear_project_roots()
        .env_source(Arc::new(
            MapEnv::new()
                .with_var("XDG_CONFIG_HOME", temp.path().join("xdg-home"))
                .with_var("HOME", &empty_home),
        ))
        .build();

    Ok(capture_metadata(|| {
        discovery.compose_scoped_layers(AutomaticMode::StackScopes, &[DiscoveryScope::User])
    }))
}

/// The automatic branch's policy event declares no `selector_class` at all.
///
/// This is the `discovery.policy` half of the contract: an explicit selector
/// carries a class, the automatic branch carries none, and the distinction is
/// the whole point of the event. A subscriber that filters on
/// `selector_class` must not match the automatic branch.
#[test]
fn the_automatic_policy_event_declares_no_selector_class() {
    let policy = policy_with(MapEnv::new());

    let events = capture_metadata(|| policy.resolve_layers());
    let policy_event = only_metadata(&events, "discovery.policy");
    assert!(
        !policy_event.declares("selector_class"),
        "the automatic branch must omit the name, not merely its value; declared: {:?}",
        policy_event.names().collect::<Vec<_>>()
    );
    assert!(
        policy_event.declares("operation"),
        "the operation field must still be declared: {:?}",
        policy_event.names().collect::<Vec<_>>()
    );
}

/// A winning selector's policy event declares `selector_class`.
///
/// The counterpart to the case above, and the guard against "omit the field
/// always" passing as a fix: the explicit branch must still carry the name.
#[test]
fn a_selector_policy_event_declares_selector_class() {
    let policy = policy_with(MapEnv::new())
        .selectors([ConfigPathSelector::cli(Some(PathBuf::from("missing.toml")))]);

    let events = capture_metadata(|| policy.resolve_layers());
    let policy_event = only_metadata(&events, "discovery.policy");
    assert!(
        policy_event.declares("selector_class"),
        "a selector win must declare selector_class; declared: {:?}",
        policy_event.names().collect::<Vec<_>>()
    );
}

/// A `not_found` load event declares no `source` and no `scope`.
///
/// Both are absent for the flat first-wins walk whether or not it found a
/// winner, so this is the case a collapsed emitter silently gets wrong.
#[test]
fn an_unsuccessful_flat_load_declares_neither_source_nor_scope() {
    let discovery = ConfigDiscovery::builder("demo")
        .clear_project_roots()
        .env_source(Arc::new(MapEnv::new()))
        .build();

    let events = capture_metadata(|| discovery.load_first());
    let load = only_metadata(&events, "discovery.load");
    assert!(
        !load.declares("source"),
        "a flat not_found event must not declare source; declared: {:?}",
        load.names().collect::<Vec<_>>()
    );
    assert!(
        !load.declares("scope"),
        "a flat walk must not declare scope; declared: {:?}",
        load.names().collect::<Vec<_>>()
    );
    assert!(load.declares("operation") && load.declares("outcome"));
}

/// A successful source-carrying flat load declares `source`, never `scope`.
///
/// The middle case of the four spellings: it separates "no scope in this walk"
/// from "no winner in this run", which a single collapsed emitter cannot.
#[test]
fn a_successful_flat_load_declares_source_but_not_scope() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let path = write_fixture(dir.path(), "selected.toml", "value = 1\n")
        .expect("fixture should be written");

    let discovery = ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .clear_project_roots()
        .env_source(Arc::new(MapEnv::new().with_var("DEMO_CONFIG", &path)))
        .build();

    let events = capture_metadata(|| discovery.load_first());
    let load = only_metadata(&events, "discovery.load");
    assert!(
        load.declares("source"),
        "a winning flat load must declare source; declared: {:?}",
        load.names().collect::<Vec<_>>()
    );
    assert!(
        !load.declares("scope"),
        "the flat walk must not declare scope even on success; declared: {:?}",
        load.names().collect::<Vec<_>>()
    );
}

/// A policy `load` event declares neither optional field.
///
/// `policy_resolve` consults no candidate list, so `source` is documented as
/// omitted; `scope` belongs to the scoped walk. This is the third distinct
/// spelling of the same event name.
#[test]
fn a_policy_load_declares_neither_source_nor_scope() {
    let policy = policy_with(MapEnv::new())
        .selectors([ConfigPathSelector::cli(Some(PathBuf::from("missing.toml")))]);

    let events = capture_metadata(|| policy.resolve_layers());
    let load = only_metadata(&events, "discovery.load");
    assert!(
        !load.declares("source"),
        "policy_resolve must not declare source; declared: {:?}",
        load.names().collect::<Vec<_>>()
    );
    assert!(
        !load.declares("scope"),
        "policy_resolve must not declare scope; declared: {:?}",
        load.names().collect::<Vec<_>>()
    );
}

/// An unscoped candidate failure declares no `scope`.
///
/// The flat first-wins walk reports failures with `scope: None`, and the name
/// must be absent from the callsite rather than merely unrecorded.
#[test]
fn a_flat_candidate_failure_declares_no_scope() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let path =
        write_fixture(dir.path(), "bad.toml", "value = ???\n").expect("fixture should be written");

    let discovery = ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .clear_project_roots()
        .env_source(Arc::new(MapEnv::new().with_var("DEMO_CONFIG", &path)))
        .build();

    let events = capture_metadata(|| drop(discovery.load_first_with_errors()));
    let failures = all_metadata(&events, "discovery.candidate");
    assert!(
        !failures.is_empty(),
        "the malformed selector candidate must fail, or this case proves nothing"
    );
    for failure in failures {
        assert!(
            !failure.declares("scope"),
            "a flat candidate failure must not declare scope; declared: {:?}",
            failure.names().collect::<Vec<_>>()
        );
        assert!(
            failure.declares("source") && failure.declares("category"),
            "the bounded fields must still be declared: {:?}",
            failure.names().collect::<Vec<_>>()
        );
    }
}

/// A scoped candidate failure declares `scope`, proving the split is real.
///
/// Together with the case above this pins both arms of the conditional: the
/// emitter that omits `scope` and the emitter that carries it are separate
/// callsites, which is what keeps the documentation's "present only when a
/// scoped walk met the candidate" true of metadata.
#[test]
fn a_scoped_candidate_failure_declares_scope() {
    let events = run_user_scope("value = ???\n").expect("the user-scope fixture is preparable");
    let failures = all_metadata(&events, "discovery.candidate");
    assert!(
        !failures.is_empty(),
        "the malformed user-scope candidate must fail, or this case proves nothing"
    );
    for failure in failures {
        assert!(
            failure.declares("scope"),
            "a scoped candidate failure must declare scope; declared: {:?}",
            failure.names().collect::<Vec<_>>()
        );
    }
}

/// A scoped winning load declares both `source` and `scope`.
///
/// The fourth spelling, and the one that makes the other three meaningful: if
/// every emitter declared both names, only a `scope`-filtering subscriber would
/// notice, and the documentation would be wrong without any value-level test
/// failing.
#[test]
fn a_scoped_winning_load_declares_both_source_and_scope() {
    let events = run_user_scope("value = 1\n").expect("the user-scope fixture is preparable");
    let loads: Vec<_> = all_metadata(&events, "discovery.load")
        .into_iter()
        .filter(|event| event.declares("scope"))
        .collect();
    assert!(
        !loads.is_empty(),
        "the user scope must produce a winning load, or this case proves nothing"
    );
    for load in loads {
        assert!(
            load.declares("source"),
            "declared: {:?}",
            load.names().collect::<Vec<_>>()
        );
    }
}
