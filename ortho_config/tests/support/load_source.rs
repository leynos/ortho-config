//! Which rung won a successful discovery.
//!
//! The terminal `discovery.load` event reports `outcome = "success"`, but an
//! operator asking "why did it load *that* file?" needs to know which of the
//! dozen consulted locations produced it. These cases drive one rung each to
//! the front of the search order and pin the `source` label the event reports.
//!
//! Every case injects its environment through `MapEnv` and writes its fixture
//! into a temporary directory, so nothing here reads or mutates the process
//! environment and no case depends on another's state.

use super::capture_support::{capture, only, write_fixture};
use super::discovery_with;
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{ConfigDiscovery, MapEnv};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Create `relative` beneath `base`, returning the resulting directory.
///
/// Goes through a `cap_std::fs::Dir` handle for the same reason
/// [`write_fixture`] does: the capability names the directory it may touch, so
/// a fixture cannot escape the temporary tree it was given.
fn make_dir(base: &Path, relative: &Path) -> anyhow::Result<PathBuf> {
    let cap = Dir::open_ambient_dir(base, ambient_authority())?;
    cap.create_dir_all(relative)?;
    Ok(base.join(relative))
}

/// An `XDG_CONFIG_HOME` candidate reports `xdg`.
///
/// The fixture goes at the exact nested candidate path, `<base>/demo/config.toml`,
/// and no selector is set, so the XDG rung is the first that can match.
#[test]
fn an_xdg_candidate_reports_the_xdg_source() {
    let base = tempfile::tempdir().expect("a temporary directory should be creatable");
    let nested = make_dir(base.path(), Path::new("demo")).expect("the app directory is creatable");
    write_fixture(&nested, "config.toml").expect("fixture should be written");

    let discovery = discovery_with(MapEnv::new().with_var("XDG_CONFIG_HOME", base.path()));
    let events = capture(|| discovery.load_first());

    let load = only(&events, "discovery.load");
    assert_eq!(load.field("operation"), "discover_first");
    assert_eq!(load.field("outcome"), "success");
    assert_eq!(load.field("source"), "xdg");
}

/// A home-directory candidate reports `home`.
///
/// `XDG_CONFIG_DIRS` is pinned to an empty temporary directory so the XDG rung
/// resolves to a list that matches nothing, rather than to the platform's
/// `/etc/xdg` default — which would make the case depend on the host.
#[test]
fn a_home_candidate_reports_the_home_source() {
    let home = tempfile::tempdir().expect("a temporary directory should be creatable");
    let nested = make_dir(home.path(), &Path::new(".config").join("demo"))
        .expect("the home config directory is creatable");
    write_fixture(&nested, "config.toml").expect("fixture should be written");
    let empty_xdg = tempfile::tempdir().expect("a temporary directory should be creatable");

    let discovery = discovery_with(
        MapEnv::new()
            .with_var("HOME", home.path())
            .with_var("XDG_CONFIG_DIRS", empty_xdg.path()),
    );
    let events = capture(|| discovery.load_first());

    let load = only(&events, "discovery.load");
    assert_eq!(load.field("operation"), "discover_first");
    assert_eq!(load.field("outcome"), "success");
    assert_eq!(load.field("source"), "home");
}

/// A project-root candidate reports `project`.
///
/// The project rung is last, so an empty `MapEnv` is enough to clear every
/// earlier one: with no variables set there is no selector, no XDG base, no
/// Windows application-data directory, and no home.
#[test]
fn a_project_candidate_reports_the_project_source() {
    let root = tempfile::tempdir().expect("a temporary directory should be creatable");
    write_fixture(root.path(), ".demo.toml").expect("fixture should be written");

    let discovery = ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .clear_project_roots()
        .add_project_root(root.path())
        .env_source(Arc::new(MapEnv::new()))
        .build();
    let events = capture(|| discovery.load_first());

    let load = only(&events, "discovery.load");
    assert_eq!(load.field("operation"), "discover_first");
    assert_eq!(load.field("outcome"), "success");
    assert_eq!(load.field("source"), "project");
}

/// `compose_layers` reports the winning rung too.
///
/// It shares `walk_candidates` with `load_first` but reaches it by its own
/// path, so the field is pinned for both operations rather than assumed to
/// follow from one.
#[test]
fn compose_layers_reports_the_winning_source() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let path = write_fixture(dir.path(), "selected.toml").expect("fixture should be written");

    let discovery = discovery_with(MapEnv::new().with_var("DEMO_CONFIG", &path));
    let events = capture(|| discovery.compose_layers());

    let load = only(&events, "discovery.load");
    assert_eq!(load.field("operation"), "compose_layers");
    assert_eq!(load.field("outcome"), "success");
    assert_eq!(load.field("source"), "selector");
}

/// A terminal outcome with no winner carries no `source`.
///
/// The asymmetry is the design: `source` names a candidate, and `not_found`
/// means there was none. An empty field here is what stops a consumer reading
/// a stale or invented rung out of an unsuccessful event.
#[test]
fn an_unsuccessful_load_reports_no_source() {
    let discovery = discovery_with(MapEnv::new());
    let events = capture(|| discovery.load_first());

    let load = only(&events, "discovery.load");
    assert_eq!(load.field("outcome"), "not_found");
    assert_eq!(load.field("source"), "");
}
