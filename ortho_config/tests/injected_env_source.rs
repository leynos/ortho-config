//! Discovery driven entirely by an injected environment source.
//!
//! These tests deliberately mutate nothing. They run concurrently by default —
//! no `#[serial]`, no environment lock, no `figment::Jail` — which is the
//! property the injected source exists to provide. Adding a process mutation to
//! this file would silently reintroduce the coupling it was written to
//! demonstrate is gone.
//!
//! The assertions are *exact sequences*, not containment checks. Discovery's
//! contract is precedence: which location wins when two of them hold a file.
//! A `candidates().iter().any(...)` assertion is satisfied whether the
//! candidate is first or last, so it cannot fail when a base directory
//! overtakes the selector — the one regression worth catching. Expectations are
//! built from `support/discovery_expect.rs` so they stay correct across the
//! `json5` and `yaml` feature combinations.

use anyhow::Context as _;
use cap_std::{ambient_authority, fs::Dir};
use ortho_config::{ConfigDiscovery, EnvSource, MapEnv};
use rstest::rstest;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "support/discovery_expect.rs"]
mod expect;

use expect::{
    APP, DOTFILE, PROJECT_ROOT, SELECTOR, assert_contains_run, assert_precedes, base_group,
    default_xdg_group, home_group, project_group, sequence,
};

fn discovery_with(env: MapEnv) -> ConfigDiscovery {
    ConfigDiscovery::builder(APP)
        .env_var(SELECTOR)
        .clear_project_roots()
        .add_project_root(Path::new(PROJECT_ROOT))
        .env_source(Arc::new(env))
        .build()
}

fn candidates_for(pairs: &[(&str, &str)]) -> Vec<PathBuf> {
    discovery_with(pairs.iter().copied().collect()).candidates()
}

/// Write the file the selector will name, returning its path.
///
/// The write goes through a `cap_std::fs::Dir` handle rather than `std::fs`,
/// as the repository's lint suite requires: a capability handle names the
/// directory it may touch, so a fixture cannot write relative to the process's
/// working directory by accident.
fn write_selector_fixture(dir: &Path, contents: &[u8]) -> anyhow::Result<PathBuf> {
    let cap =
        Dir::open_ambient_dir(dir, ambient_authority()).context("open the temporary directory")?;
    cap.write("selected.toml", contents)
        .context("write the fixture")?;
    Ok(dir.join("selected.toml"))
}

/// The selector leads the list, ahead of every platform and project location.
#[test]
fn selector_precedes_every_other_candidate() {
    let selected = PathBuf::from("/etc/selected.toml");
    let actual = candidates_for(&[(SELECTOR, "/etc/selected.toml")]);

    assert_eq!(
        actual,
        sequence([vec![selected], default_xdg_group(), project_group(),]),
    );
}

/// An empty selector is indistinguishable from an unset one.
///
/// Comparing the two whole lists is what makes this a real claim. Asserting
/// merely that no *empty* path appears would also pass if the empty selector
/// had silently displaced a later candidate, because `push_unique` rejects an
/// empty path on its own account — the guard would be doing the work while the
/// selector logic went untested.
#[test]
fn an_empty_selector_yields_the_same_list_as_an_absent_one() {
    let with_empty = candidates_for(&[(SELECTOR, "")]);
    let without = candidates_for(&[]);

    assert_eq!(with_empty, without);

    let expected = sequence([default_xdg_group(), project_group()]);
    assert_eq!(with_empty, expected);
    assert!(
        with_empty.contains(&PathBuf::from(PROJECT_ROOT).join(DOTFILE)),
        "the project candidate must survive an empty selector, got {with_empty:?}"
    );
}

/// `XDG_CONFIG_HOME` outranks every other base directory and the project root.
#[test]
fn xdg_config_home_precedes_the_other_base_directories() {
    let dirs = std::env::join_paths(["/xdg-a"]).expect("the directory list should join");
    let actual = candidates_for(&[
        ("XDG_CONFIG_HOME", "/xdg-home"),
        (
            "XDG_CONFIG_DIRS",
            dirs.to_str().expect("the joined list should be UTF-8"),
        ),
        ("APPDATA", "/appdata"),
        ("HOME", "/home/injected"),
    ]);

    assert_eq!(
        actual,
        sequence([
            base_group("/xdg-home"),
            base_group("/xdg-a"),
            base_group("/appdata"),
            home_group("/home/injected"),
            project_group(),
        ]),
    );
}

/// Injected `XDG_CONFIG_DIRS` entries keep their order and displace the default.
#[test]
fn xdg_config_dirs_are_used_in_order_and_replace_the_default() {
    let joined = std::env::join_paths(["/xdg-first", "/xdg-second"])
        .expect("the directory list should join");
    let actual = candidates_for(&[(
        "XDG_CONFIG_DIRS",
        joined.to_str().expect("the joined list should be UTF-8"),
    )]);

    assert_eq!(
        actual,
        sequence([
            base_group("/xdg-first"),
            base_group("/xdg-second"),
            project_group(),
        ]),
    );

    for defaulted in default_xdg_group() {
        assert!(
            !actual.contains(&defaulted),
            "the default XDG group must not appear alongside an injected list: {defaulted:?}"
        );
    }
}

/// An `XDG_CONFIG_DIRS` with no usable entry falls back to the platform default.
///
/// Gated to the platforms that *have* a default: elsewhere `push_default_xdg`
/// is deliberately a no-op, so there would be nothing to assert.
#[cfg(any(unix, target_os = "redox"))]
#[rstest]
#[case::empty("")]
#[case::only_separators(":")]
fn an_unusable_xdg_dirs_list_falls_back_to_the_default(#[case] value: &str) {
    let actual = candidates_for(&[("XDG_CONFIG_DIRS", value)]);
    assert_eq!(actual, sequence([default_xdg_group(), project_group()]));
}

/// `APPDATA` precedes `LOCALAPPDATA`, matching the Windows search order.
#[test]
fn appdata_precedes_localappdata() {
    let actual = candidates_for(&[("APPDATA", "/appdata"), ("LOCALAPPDATA", "/localappdata")]);

    assert_eq!(
        actual,
        sequence([
            default_xdg_group(),
            base_group("/appdata"),
            base_group("/localappdata"),
            project_group(),
        ]),
    );
}

/// `HOME` wins over `USERPROFILE`, and `USERPROFILE` is used when alone.
#[rstest]
#[case::home_wins(&[("HOME", "/home/injected"), ("USERPROFILE", "/users/injected")], "/home/injected")]
#[case::userprofile_alone(&[("USERPROFILE", "/users/injected")], "/users/injected")]
fn the_home_ladder_picks_one_directory(#[case] pairs: &[(&str, &str)], #[case] expected: &str) {
    let actual = candidates_for(pairs);

    assert_eq!(
        actual,
        sequence([default_xdg_group(), home_group(expected), project_group(),]),
    );
}

/// With no home in the source, no host home may leak into the candidate list.
///
/// This is the property that makes the suite machine-independent: the platform
/// `home_dir()` fallback must be suppressed for an injected source, or the
/// candidates would differ between developer machines and CI. Asserting the
/// exact sequence proves it more directly than the previous prefix check —
/// there is simply nowhere for a host path to be.
#[test]
fn absent_home_does_not_fall_back_to_the_host() {
    let actual = candidates_for(&[]);
    assert_eq!(actual, sequence([default_xdg_group(), project_group()]));

    if let Some(host) = dirs::home_dir() {
        assert!(
            !actual.iter().any(|path| path.starts_with(&host)),
            "host home {host:?} leaked into {actual:?}"
        );
    }
}

/// An empty home variable is treated as unset, and does not block the next one.
///
/// `PathBuf::from("")` joined with `.config` yields a *relative* path, so an
/// empty `HOME` would search the process's working directory. It must also not
/// mask a populated `USERPROFILE`: an operator who exports an empty `HOME` has
/// said nothing about where the home is, not that there is none.
#[rstest]
#[case::empty_home(&[("HOME", "")], None)]
#[case::empty_userprofile(&[("USERPROFILE", "")], None)]
#[case::both_empty(&[("HOME", ""), ("USERPROFILE", "")], None)]
#[case::empty_home_falls_through(
    &[("HOME", ""), ("USERPROFILE", "/users/injected")],
    Some("/users/injected")
)]
fn an_empty_home_variable_is_treated_as_unset(
    #[case] pairs: &[(&str, &str)],
    #[case] expected_home: Option<&str>,
) {
    let actual = candidates_for(pairs);
    let home_group = expected_home.map_or_else(Vec::new, home_group);
    assert_eq!(
        actual,
        sequence([default_xdg_group(), home_group, project_group()]),
    );
}

/// A `MapEnv` models a closed set, so unknown keys are unset.
#[test]
fn map_env_reports_absent_keys_as_unset() {
    let env = MapEnv::new().with_var("PRESENT", "1");
    assert!(env.get("XDG_CONFIG_HOME").is_none());
    assert!(env.home_fallback().is_none());
}

/// An empty base-directory variable must contribute no candidate.
///
/// `PathBuf::from("")` joined with the app name yields a *relative* path such
/// as `demo/config.toml`. Left unguarded, that would be resolved against the
/// process's working directory, so a tool run from an untrusted directory
/// could load configuration from it. The exact-sequence assertion is what
/// pins this: a relative candidate has nowhere to hide in it.
#[rstest]
#[case::xdg_config_home("XDG_CONFIG_HOME")]
#[case::appdata("APPDATA")]
#[case::localappdata("LOCALAPPDATA")]
fn empty_base_directory_contributes_no_candidate(#[case] key: &str) {
    let actual = candidates_for(&[(key, "")]);
    assert_eq!(actual, sequence([default_xdg_group(), project_group()]));
}

/// Every base directory contributes its group, in the documented ladder order.
#[test]
fn the_full_ladder_is_ordered_selector_xdg_windows_home_project() {
    let joined = std::env::join_paths(["/xdg-dirs-a", "/xdg-dirs-b"])
        .expect("the directory list should join");
    let actual = candidates_for(&[
        (SELECTOR, "/etc/selected.toml"),
        ("XDG_CONFIG_HOME", "/xdg-home"),
        (
            "XDG_CONFIG_DIRS",
            joined.to_str().expect("the joined list should be UTF-8"),
        ),
        ("APPDATA", "/appdata"),
        ("LOCALAPPDATA", "/localappdata"),
        ("HOME", "/home/injected"),
    ]);

    assert_eq!(
        actual,
        sequence([
            vec![PathBuf::from("/etc/selected.toml")],
            base_group("/xdg-home"),
            base_group("/xdg-dirs-a"),
            base_group("/xdg-dirs-b"),
            base_group("/appdata"),
            base_group("/localappdata"),
            home_group("/home/injected"),
            project_group(),
        ]),
    );

    // The same claim stated as pairwise precedence, so a failure names the two
    // locations that swapped rather than dumping the whole list.
    assert_precedes(
        &actual,
        Path::new("/etc/selected.toml"),
        &PathBuf::from("/xdg-home").join(APP).join("config.toml"),
    );
    assert_precedes(
        &actual,
        &PathBuf::from("/appdata").join(APP).join("config.toml"),
        &PathBuf::from("/home/injected")
            .join(".config")
            .join(APP)
            .join("config.toml"),
    );
    assert_contains_run(&actual, &home_group("/home/injected"));
}

/// A file named by the injected selector is the one that loads.
///
/// This closes the loop the candidate tests leave open: they prove the
/// selector's path is first in the list, not that discovery goes on to open
/// it. Both are needed — a correct order that never reaches the filesystem
/// would satisfy every assertion above.
#[test]
fn the_injected_selector_names_the_file_that_loads() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let selected = write_selector_fixture(dir.path(), br#"recipient = "injected""#)
        .expect("the fixture should be written");

    let discovery = discovery_with(MapEnv::new().with_var(SELECTOR, &selected));

    let candidates = discovery.candidates();
    assert_eq!(
        candidates.first().map(PathBuf::as_path),
        Some(selected.as_path()),
        "the selected file must precede every fallback, got {candidates:?}"
    );

    let figment = discovery
        .load_first()
        .expect("discovery should not error")
        .expect("the selected file should have loaded");
    let recipient: String = figment
        .extract_inner("recipient")
        .expect("the loaded figment should carry the fixture value");
    assert_eq!(recipient, "injected");
}

/// The same file reaches `compose_layer`, tagged with its own path.
#[test]
fn the_injected_selector_composes_a_layer() {
    let dir = tempfile::tempdir().expect("a temporary directory should be creatable");
    let selected = write_selector_fixture(dir.path(), br#"recipient = "layered""#)
        .expect("the fixture should be written");

    let discovery = discovery_with(MapEnv::new().with_var(SELECTOR, &selected));
    let outcome = discovery.compose_layer();

    assert!(
        outcome.required_errors.is_empty(),
        "no required candidate was configured, got {:?}",
        outcome.required_errors
    );
    let layer = outcome.value.expect("a layer should have been composed");
    assert_eq!(
        layer.path().map(camino::Utf8Path::as_str),
        selected.to_str(),
        "the layer must name the file it came from"
    );
}
