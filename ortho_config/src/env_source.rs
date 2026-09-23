//! Injectable environment access for configuration discovery.
//!
//! Discovery reads several environment variables — the configuration-path
//! selector, the XDG base directories, the Windows application-data folders,
//! and the user's home directory. Reading them straight from the process means
//! a test can only influence discovery by mutating global state, which forces
//! the whole suite behind a lock and rules out property-based testing.
//!
//! [`EnvSource`] abstracts lookup during discovery, while [`ScanEnvSource`]
//! is the separate, explicit merge-layer scanning capability completed by
//! issue #412. [`ProcessEnv`] is the default and preserves existing behaviour
//! exactly; [`MapEnv`] supplies a fixed set of values for tests and embedding.
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

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::sync::Arc;

#[cfg(not(any(unix, target_os = "redox")))]
use directories::BaseDirs;

/// Read-only environment access used during configuration discovery.
///
/// The trait is deliberately object-safe so it can be held as
/// `Arc<dyn EnvSource>` without making every consumer generic.
///
/// Lookup is **by name only**. There is deliberately no method to enumerate
/// the environment, because RFC 0001 makes "the crate never scans the whole
/// process environment" a safety property of environment access: a process
/// holding thousands of unrelated secrets must never have them enumerated,
/// bulk-copied, or bulk-logged. A caller that knows a variable's name may
/// still request, copy, or log that individual value. An enumeration method
/// here would void the undirected-enumeration guarantee for every holder of an
/// `EnvSource`, however carefully individual callers behaved.
///
/// The `CsvEnv` merge layer does legitimately scan a prefix, because that is
/// what `figment::providers::Env` does. Its injectable path is represented by
/// [`ScanEnvSource`], so the scanning operation stays visible in the type
/// rather than latent in a trait whose other users must not scan.
pub trait EnvSource: fmt::Debug + Send + Sync {
    /// Return the value of `key`, or `None` when it is unset.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{EnvSource, MapEnv};
    ///
    /// let env = MapEnv::new().with_var("APP_HOST", "localhost");
    /// assert_eq!(env.get("APP_HOST").as_deref(), Some("localhost".as_ref()));
    /// assert!(env.get("APP_PORT").is_none());
    /// ```
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
    /// existing platform fallback, and custom sources may override it too.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{EnvSource, MapEnv};
    ///
    /// // A closed-set source keeps the default: no host home leaks in.
    /// assert!(MapEnv::new().home_fallback().is_none());
    /// ```
    fn home_fallback(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Return the platform configuration directory when named values are insufficient.
    ///
    /// Only non-Unix and non-Redox subcommand discovery uses this fallback.
    /// The default keeps injected sources closed over their supplied values, so
    /// a test does not accidentally load configuration from the host's native
    /// platform directory. [`ProcessEnv`] preserves the established
    /// `directories::BaseDirs` behaviour, while custom sources can supply a
    /// native path without reintroducing process access.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{EnvSource, MapEnv};
    ///
    /// assert!(MapEnv::new().config_dir_fallback().is_none());
    /// ```
    fn config_dir_fallback(&self) -> Option<std::path::PathBuf> {
        None
    }
}

/// Enumerate variables for a configuration merge layer.
///
/// This trait is deliberately separate from [`EnvSource`]. RFC 0001 §5.3
/// makes the absence of enumeration from `EnvSource` a safety property: a
/// discovery caller cannot accidentally scan unrelated process secrets.
/// [`crate::CsvEnv`] must scan a prefix to preserve
/// [`figment::providers::Env`] semantics, so its callers opt into that broader
/// capability explicitly with `ScanEnvSource`.
pub trait ScanEnvSource: fmt::Debug + Send + Sync {
    /// Return every variable as an owned native key/value pair.
    fn scan(&self) -> Vec<(OsString, OsString)>;
}

/// Shorthand for a shared environment source.
pub type SharedEnvSource = Arc<dyn EnvSource>;

/// Shorthand for a shared environment source that permits enumeration.
pub type SharedScanEnvSource = Arc<dyn ScanEnvSource>;

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

    #[cfg(not(any(unix, target_os = "redox")))]
    fn config_dir_fallback(&self) -> Option<std::path::PathBuf> {
        BaseDirs::new().map(|dirs| dirs.config_dir().to_path_buf())
    }
}

impl ScanEnvSource for ProcessEnv {
    fn scan(&self) -> Vec<(OsString, OsString)> {
        std::env::vars_os().collect()
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
    // `HashMap`: lookups are name-only and nothing iterates the map, so
    // ordering would buy determinism no output consumes at O(log n) cost.
    vars: HashMap<String, OsString>,
}

impl MapEnv {
    /// Create an empty source, in which every variable is unset.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{EnvSource, MapEnv};
    ///
    /// let env = MapEnv::new();
    /// assert!(env.get("ANYTHING").is_none());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one variable, consuming and returning `self` for chaining.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{EnvSource, MapEnv};
    ///
    /// let env = MapEnv::new().with_var("APP_PORT", "8080");
    /// assert_eq!(env.get("APP_PORT").as_deref(), Some("8080".as_ref()));
    /// ```
    #[must_use]
    pub fn with_var(mut self, key: impl Into<String>, value: impl AsRef<OsStr>) -> Self {
        self.vars.insert(key.into(), value.as_ref().to_os_string());
        self
    }

    /// Add one variable in place.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{EnvSource, MapEnv};
    ///
    /// let mut env = MapEnv::new();
    /// env.insert("APP_HOST", "localhost");
    /// assert_eq!(env.get("APP_HOST").as_deref(), Some("localhost".as_ref()));
    /// ```
    pub fn insert(&mut self, key: impl Into<String>, value: impl AsRef<OsStr>) {
        self.vars.insert(key.into(), value.as_ref().to_os_string());
    }

    /// Remove one variable, so lookups report it as unset.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ortho_config::{EnvSource, MapEnv};
    ///
    /// let mut env = MapEnv::new().with_var("APP_HOST", "localhost");
    /// env.remove("APP_HOST");
    /// assert!(env.get("APP_HOST").is_none());
    /// ```
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

impl ScanEnvSource for MapEnv {
    fn scan(&self) -> Vec<(OsString, OsString)> {
        self.vars
            .iter()
            .map(|(key, value)| (OsString::from(key), value.clone()))
            .collect()
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
///
/// # Examples
///
/// ```rust
/// use ortho_config::process_env_source;
///
/// // Reads flow through to the live process environment, so the source
/// // answers exactly what the process itself sees.
/// let source = process_env_source();
/// assert_eq!(source.get("PATH"), std::env::var_os("PATH"));
/// ```
#[must_use]
pub fn process_env_source() -> SharedEnvSource {
    Arc::new(ProcessEnv)
}

/// Return the default process-backed scanning environment source.
///
/// # Examples
///
/// ```rust
/// use ortho_config::process_scan_env_source;
///
/// let source = process_scan_env_source();
/// let _variables = source.scan();
/// ```
#[must_use]
pub fn process_scan_env_source() -> SharedScanEnvSource {
    Arc::new(ProcessEnv)
}

#[cfg(test)]
#[path = "env_source_tests.rs"]
mod tests;
