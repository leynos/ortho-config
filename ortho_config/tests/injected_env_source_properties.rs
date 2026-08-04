//! Property coverage for the injected environment source.
//!
//! The example-based suite in `injected_env_source.rs` pins specific ladders;
//! these properties state the invariants those examples are instances of, over
//! inputs a reviewer would not think to write down.
//!
//! Every case is generated from the proptest seed alone. Nothing here reads or
//! mutates the process environment — that is what makes property testing
//! possible at all for this code. A generator that had to `set_var` would need
//! the whole suite behind a lock, and a thousand serialized cases is not a
//! property test anyone runs.

use ortho_config::{ConfigDiscovery, MapEnv};
use proptest::collection::vec;
use proptest::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[path = "support/discovery_expect.rs"]
mod expect;

use expect::{
    APP, DOTFILE, PROJECT_ROOT, SELECTOR, assert_contains_run, assert_precedes, base_group,
    default_xdg_group, home_group, project_group, sequence,
};

/// Every variable discovery consults. A generated key must avoid all of them,
/// or "unrelated key" would not be unrelated.
const CONSULTED: &[&str] = &[
    SELECTOR,
    "XDG_CONFIG_HOME",
    "XDG_CONFIG_DIRS",
    "APPDATA",
    "LOCALAPPDATA",
    "HOME",
    "USERPROFILE",
];

fn discovery_with(env: MapEnv) -> ConfigDiscovery {
    ConfigDiscovery::builder(APP)
        .env_var(SELECTOR)
        .clear_project_roots()
        .add_project_root(Path::new(PROJECT_ROOT))
        .env_source(Arc::new(env))
        .build()
}

/// The candidate list when the source supplies nothing at all.
fn baseline() -> Vec<PathBuf> {
    sequence([default_xdg_group(), project_group()])
}

/// A variable name discovery does not consult.
fn unrelated_key() -> impl Strategy<Value = String> {
    "[A-Z][A-Z_]{0,11}".prop_filter("must not be a consulted variable", |key| {
        !CONSULTED.contains(&key.as_str())
    })
}

/// A single path segment: no separator, no empty string, no `.` or `..`.
fn path_segment() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9-]{0,7}".prop_map(|segment| segment)
}

proptest! {
    /// Injecting variables that discovery does not read changes nothing.
    ///
    /// This is the containment half of the trait's contract: `EnvSource::get`
    /// is by-name only, so a source holding a thousand unrelated secrets must
    /// influence discovery exactly as much as an empty one.
    #[test]
    fn unrelated_keys_do_not_change_the_candidates(
        pairs in vec((unrelated_key(), "[a-z/]{0,16}"), 0..8),
    ) {
        let env: MapEnv = pairs.into_iter().collect();
        prop_assert_eq!(discovery_with(env).candidates(), baseline());
    }

    /// An empty selector is exactly an absent selector, for any surrounding state.
    #[test]
    fn an_empty_selector_matches_an_absent_selector(
        home in path_segment(),
        xdg in path_segment(),
    ) {
        let base = MapEnv::new()
            .with_var("HOME", format!("/home/{home}"))
            .with_var("XDG_CONFIG_HOME", format!("/xdg/{xdg}"));

        let with_empty = discovery_with(base.clone().with_var(SELECTOR, "")).candidates();
        let without = discovery_with(base).candidates();

        prop_assert_eq!(with_empty, without);
    }

    /// Without `HOME` or `USERPROFILE`, the platform home fallback is not used.
    ///
    /// `ProcessEnv` overrides `home_fallback` to consult the real user
    /// database; `MapEnv` does not. If discovery ever reached past the source
    /// to the platform, this property would fail on any machine with a home
    /// directory — which is every machine that runs it, so the property is not
    /// vacuous by construction.
    #[test]
    fn a_source_without_a_home_never_reaches_the_platform(
        pairs in vec((unrelated_key(), "[a-z/]{0,16}"), 0..4),
    ) {
        let env: MapEnv = pairs.into_iter().collect();
        let candidates = discovery_with(env).candidates();

        prop_assert_eq!(&candidates, &baseline());
        if let Some(host) = dirs::home_dir() {
            prop_assert!(
                !candidates.iter().any(|path| path.starts_with(&host)),
                "host home {:?} leaked into {:?}",
                host,
                candidates,
            );
        }
    }

    /// `XDG_CONFIG_DIRS` groups appear in the order the list gave them.
    #[test]
    fn xdg_directories_keep_their_input_order(
        segments in vec(path_segment(), 1..4).prop_filter(
            "segments must be distinct",
            |segments| {
                let mut sorted = segments.clone();
                sorted.sort();
                sorted.dedup();
                sorted.len() == segments.len()
            },
        ),
    ) {
        let dirs: Vec<PathBuf> = segments
            .iter()
            .map(|segment| PathBuf::from(format!("/xdg/{segment}")))
            .collect();
        let joined = std::env::join_paths(&dirs).expect("generated segments must join");

        let env = MapEnv::new().with_var("XDG_CONFIG_DIRS", &joined);
        let candidates = discovery_with(env).candidates();

        let expected = sequence(
            dirs.iter()
                .map(base_group)
                .chain(std::iter::once(project_group())),
        );
        prop_assert_eq!(candidates, expected);
    }

    /// A non-UTF-8 selector survives `candidates` and is dropped by `utf8_candidates`.
    ///
    /// Discovery carries `OsString` end to end. Transcoding lossily would keep
    /// the count intact while yielding a path that will not open, so the
    /// property asserts both halves: present natively, absent from the UTF-8
    /// view.
    #[cfg(unix)]
    #[test]
    fn non_utf8_selectors_are_preserved_and_omitted_from_the_utf8_view(
        prefix in vec(any::<u8>().prop_filter("no NUL, no separator", |byte| {
            *byte != 0 && *byte != b'/'
        }), 1..8),
    ) {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt as _;

        // 0xFF is not valid UTF-8 in any position, so the result is
        // non-representable however the generated prefix happens to decode.
        let mut raw = b"/tmp/".to_vec();
        raw.extend_from_slice(&prefix);
        raw.push(0xFF);
        let selector = OsString::from_vec(raw);
        let expected = PathBuf::from(selector.clone());

        let env = MapEnv::new().with_var(SELECTOR, &selector);
        let discovery = discovery_with(env);

        let candidates = discovery.candidates();
        prop_assert!(
            candidates.contains(&expected),
            "the native selector must survive, got {:?}",
            candidates,
        );

        let utf8 = discovery.utf8_candidates();
        prop_assert!(
            utf8.iter().all(|path| path.as_std_path() != expected),
            "the non-UTF-8 selector must not appear in {:?}",
            utf8,
        );
        prop_assert_eq!(utf8.len() + 1, candidates.len());
    }
}

/// The shared expectation helpers are exercised here too, so that a change to
/// `support/discovery_expect.rs` cannot break one suite silently while the
/// other still compiles.
#[test]
fn the_expectation_helpers_describe_the_documented_ladder() {
    let env = MapEnv::new()
        .with_var(SELECTOR, "/etc/selected.toml")
        .with_var("HOME", "/home/injected");
    let candidates = discovery_with(env).candidates();

    assert_contains_run(&candidates, &home_group("/home/injected"));
    assert_precedes(
        &candidates,
        Path::new("/etc/selected.toml"),
        &PathBuf::from(PROJECT_ROOT).join(DOTFILE),
    );
}
