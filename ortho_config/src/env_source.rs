//! Injectable environment access for configuration discovery.
//!
//! Discovery reads several environment variables — the configuration-path
//! selector, the XDG base directories, the Windows application-data folders,
//! and the user's home directory. Reading them straight from the process means
//! a test can only influence discovery by mutating global state, which forces
//! the whole suite behind a lock and rules out property-based testing.
//!
//! [`EnvSource`] abstracts that access. [`ProcessEnv`] is the default and
//! preserves existing behaviour exactly; [`MapEnv`] supplies a fixed set of
//! values for tests and embedding.
//!
//! # Examples
//!
//! ```rust
//! use ortho_config::{ConfigDiscovery, MapEnv};
//! use std::sync::Arc;
//!
//! let env = Arc::new(MapEnv::new().with_var("DEMO_CONFIG", "/etc/demo.toml"));
//! let discovery = ConfigDiscovery::builder("demo")
//!     .env_var("DEMO_CONFIG")
//!     .env_source(env)
//!     .build();
//!
//! assert!(discovery.candidates().iter().any(|p| p.ends_with("demo.toml")));
//! ```

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::sync::Arc;

/// Read-only environment access used during configuration discovery.
///
/// The trait is deliberately object-safe so it can be held as
/// `Arc<dyn EnvSource>` without making every consumer generic.
///
/// Lookup is **by name only**. There is deliberately no method to enumerate
/// the environment, because RFC 0001 makes "the crate never scans the whole
/// process environment" a safety property of environment access: a process
/// holding thousands of unrelated secrets must never have them enumerated,
/// copied, or logged. An enumeration method here would void that guarantee for
/// every holder of an `EnvSource`, however carefully individual callers behaved.
///
/// The `CsvEnv` merge layer does legitimately scan a prefix, because that is
/// what `figment::providers::Env` does. When that layer gains an injectable
/// source (#412) it should take a separate, explicitly-named abstraction, so
/// that the scanning capability is visible in the type rather than latent in a
/// trait whose other users must not scan.
pub trait EnvSource: fmt::Debug + Send + Sync {
    /// Return the value of `key`, or `None` when it is unset.
    fn get(&self, key: &str) -> Option<OsString>;

    /// Return the user's home directory when the environment does not name one.
    ///
    /// Discovery falls back to a platform lookup when neither `HOME` nor
    /// `USERPROFILE` is set. That lookup consults the real user database and
    /// process environment, so an injected source must be able to suppress it —
    /// otherwise a test supplying no home would still pick up the host's, and
    /// the candidate list would vary by machine.
    ///
    /// The default returns `None`, which is correct for any source that models
    /// a closed set of variables. [`ProcessEnv`] overrides it to preserve the
    /// existing platform fallback.
    fn home_fallback(&self) -> Option<std::path::PathBuf> {
        None
    }
}

/// Shorthand for a shared environment source.
pub type SharedEnvSource = Arc<dyn EnvSource>;

/// Environment source backed by the live process environment.
///
/// This is the default for [`crate::ConfigDiscovery`], so existing callers see
/// no behavioural change.
#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        std::env::var_os(key)
    }

    fn home_fallback(&self) -> Option<std::path::PathBuf> {
        dirs::home_dir()
    }
}

/// Environment source backed by a fixed set of values.
///
/// Use this in tests instead of mutating the process environment. Because the
/// values are owned by the instance, tests using distinct `MapEnv` values are
/// independent and may run concurrently.
///
/// # Examples
///
/// ```rust
/// use ortho_config::{EnvSource, MapEnv};
///
/// let env = MapEnv::new()
///     .with_var("APP_HOST", "localhost")
///     .with_var("APP_PORT", "8080");
///
/// assert_eq!(env.get("APP_HOST").as_deref(), Some("localhost".as_ref()));
/// assert!(env.get("MISSING").is_none());
/// ```
#[derive(Default, Clone)]
pub struct MapEnv {
    // `BTreeMap` rather than `HashMap` so iteration is deterministic, keeping
    // any ordered output built from this source reproducible.
    vars: BTreeMap<String, OsString>,
}

impl MapEnv {
    /// Create an empty source, in which every variable is unset.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one variable, consuming and returning `self` for chaining.
    #[must_use]
    pub fn with_var(mut self, key: impl Into<String>, value: impl AsRef<OsStr>) -> Self {
        self.vars.insert(key.into(), value.as_ref().to_os_string());
        self
    }

    /// Add one variable in place.
    pub fn insert(&mut self, key: impl Into<String>, value: impl AsRef<OsStr>) {
        self.vars.insert(key.into(), value.as_ref().to_os_string());
    }

    /// Remove one variable, so lookups report it as unset.
    pub fn remove(&mut self, key: &str) {
        self.vars.remove(key);
    }
}

/// Debug output deliberately reveals structure only.
///
/// A `MapEnv` frequently holds secret-shaped fixtures, and `EnvSource`
/// requires `Debug`, so a derived implementation would print every key and
/// value wherever a holder is logged or unwrapped. The count is enough to
/// distinguish "empty" from "populated" in a failure message.
impl fmt::Debug for MapEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapEnv")
            .field("vars", &self.vars.len())
            .finish_non_exhaustive()
    }
}

impl EnvSource for MapEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        self.vars.get(key).cloned()
    }
}

impl<K, V> FromIterator<(K, V)> for MapEnv
where
    K: Into<String>,
    V: AsRef<OsStr>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let vars = iter
            .into_iter()
            .map(|(key, value)| (key.into(), value.as_ref().to_os_string()))
            .collect();
        Self { vars }
    }
}

/// Return the default process-backed environment source.
#[must_use]
pub fn process_env_source() -> SharedEnvSource {
    Arc::new(ProcessEnv)
}

#[cfg(test)]
mod tests {
    //! Unit tests for the in-memory environment source.
    //!
    //! Discovery-level behaviour is covered by
    //! `tests/injected_env_source.rs`; these cases pin `MapEnv`'s own lookup
    //! semantics.

    use super::*;

    #[test]
    fn map_env_reports_unset_variables_as_none() {
        let env = MapEnv::new().with_var("PRESENT", "yes");
        assert!(env.get("ABSENT").is_none());
    }

    #[test]
    fn map_env_collects_from_an_iterator() {
        let env: MapEnv = [("APP_HOST", "localhost"), ("APP_PORT", "8080")]
            .into_iter()
            .collect();
        assert_eq!(env.get("APP_PORT").as_deref(), Some("8080".as_ref()));
    }

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
}
