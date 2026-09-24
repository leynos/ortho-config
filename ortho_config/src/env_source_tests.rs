//! Unit tests for the in-memory environment source.
//!
//! Discovery-level behaviour is covered by
//! `tests/injected_env_source.rs`; these cases pin `MapEnv`'s own lookup
//! semantics.

use super::*;

/// A lookup for a key that was never inserted reports the variable as unset.
#[test]
fn map_env_reports_unset_variables_as_none() {
    let env = MapEnv::new().with_var("PRESENT", "yes");
    assert!(env.get("ABSENT").is_none());
}

/// `FromIterator` builds a source whose entries are visible through `get`.
#[test]
fn map_env_collects_from_an_iterator() {
    let env: MapEnv = [("APP_HOST", "localhost"), ("APP_PORT", "8080")]
        .into_iter()
        .collect();
    assert_eq!(env.get("APP_PORT").as_deref(), Some("8080".as_ref()));
}

/// `remove` unsets a variable rather than leaving an empty value behind.
#[test]
fn map_env_remove_makes_a_variable_unset() {
    let mut env = MapEnv::new().with_var("APP_HOST", "localhost");
    env.remove("APP_HOST");
    assert!(env.get("APP_HOST").is_none());
}

/// `get` yields an owned value, not a view onto the map.
///
/// Callers hold the result across further mutation of the source, so a
/// borrowed return would either fail to compile or — were the map ever
/// swapped for interior mutability — let a later `insert` or `remove`
/// change a value already handed out.
#[test]
fn map_env_get_returns_a_value_unaffected_by_later_mutation() {
    let mut env = MapEnv::new().with_var("APP_HOST", "localhost");
    let taken = env.get("APP_HOST").expect("APP_HOST should be set");

    env.insert("APP_HOST", "replaced");
    env.remove("APP_HOST");

    assert_eq!(
        taken,
        OsString::from("localhost"),
        "the previously returned value must be owned and unchanged"
    );
    assert!(env.get("APP_HOST").is_none());
}

/// A non-UTF-8 selector reaches `candidates()` intact and is dropped by
/// `utf8_candidates()`.
///
/// Discovery carries `OsString` end to end, so a path that cannot be
/// represented as UTF-8 must survive as a native path rather than being
/// lossily transcoded; the UTF-8 view then omits it instead of returning a
/// path with replacement characters that would not open.
#[cfg(unix)]
#[test]
fn non_utf8_selector_is_preserved_natively_and_omitted_from_utf8_candidates() {
    use std::os::unix::ffi::OsStringExt as _;
    use std::path::PathBuf;

    // 0xFF is not valid UTF-8 in any position.
    let raw = OsString::from_vec(b"/etc/demo-\xFF.toml".to_vec());
    let expected = PathBuf::from(raw.clone());

    let discovery = crate::ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .clear_project_roots()
        .add_project_root(std::path::Path::new("/workspace"))
        .env_source(Arc::new(MapEnv::new().with_var("DEMO_CONFIG", &raw)))
        .build();

    let candidates = discovery.candidates();
    assert!(
        candidates.contains(&expected),
        "the native selector path must survive discovery, got {candidates:?}"
    );

    // Omission, not transcoding: every UTF-8 candidate must still be one of
    // the native candidates, and exactly one must have been dropped. A
    // lossy conversion would keep the count intact while yielding a path
    // that appears nowhere in `candidates()` and would not open.
    let utf8 = discovery.utf8_candidates();
    assert!(
        utf8.iter()
            .all(|path| candidates.iter().any(|native| native == path.as_std_path())),
        "utf8_candidates must not invent transcoded paths, got {utf8:?}"
    );
    assert_eq!(
        utf8.len() + 1,
        candidates.len(),
        "exactly the non-UTF-8 candidate must be dropped, got {utf8:?}"
    );
}
