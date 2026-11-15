//! Scoped holders for the deprecated environment guards.
//!
//! Split from `mod.rs` so each file stays within the repository's 400-line
//! ceiling. Everything here is deprecated with the rest of the module; see the
//! module documentation for the injected replacement.

#![expect(
    deprecated,
    reason = "this module implements the deprecated API and uses it internally"
)]

use super::{EnvVarGuard, EnvVarLock, lock};

/// RAII scope that holds the environment lock while retaining guards.
///
/// This is useful when a test needs to clear or set multiple environment
/// variables and keep the lock for the duration of the test to avoid
/// interleaving with other environment mutations.
///
/// # Examples
/// ```
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
///
/// let guards = vec![env::remove_var("FOO"), env::remove_var("BAR")];
/// let _scope = env::EnvScope::new(guards);
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
#[must_use = "dropping releases the environment lock and restores guards"]
pub struct EnvScope {
    /// Keeps environment mutation and guard destruction inside one locked scope.
    _lock: EnvVarLock,
    /// Restores variables in drop order before `_lock` is released.
    guards: Vec<EnvVarGuard>,
}

impl EnvScope {
    /// Create a scope that holds the global lock and retains the guards.
    ///
    /// Creating guards before calling this function can leave a window where
    /// other threads interleave between guard creation and scope construction;
    /// use helpers that build guards under the lock or pass guards directly
    /// when atomicity is required.
    pub fn new(guards: Vec<EnvVarGuard>) -> Self {
        Self {
            _lock: lock(),
            guards,
        }
    }

    /// Create a scope after running the provided builder while holding the lock.
    ///
    /// Builders must use the provided lock's methods (for example
    /// `lock.set_var`/`lock.remove_var`) instead of the standalone helpers to
    /// avoid re-entrant locking overhead.
    ///
    /// # Examples
    /// ```
    /// # #![expect(deprecated)]
    /// use ortho_config_test_helpers::env;
    ///
    /// let _scope = env::EnvScope::new_with(|lock| {
    ///     vec![lock.remove_var("FOO"), lock.remove_var("BAR")]
    /// });
    /// ```
    pub fn new_with<F>(builder: F) -> Self
    where
        F: FnOnce(&EnvVarLock) -> Vec<EnvVarGuard>,
    {
        let lock = lock();
        let guards = builder(&lock);
        Self {
            _lock: lock,
            guards,
        }
    }

    /// Create a scope from an iterator of guards.
    pub fn from_guards<I>(guards: I) -> Self
    where
        I: IntoIterator<Item = EnvVarGuard>,
    {
        Self::new(guards.into_iter().collect())
    }
}

impl Drop for EnvScope {
    fn drop(&mut self) {
        // Drop runs before field destructors, so `_lock` remains held while
        // `guards` are dropped and restore environment values.
        let guards = std::mem::take(&mut self.guards);
        drop(guards);
    }
}

/// Create a scope that holds the global lock and retains the guards.
///
/// # Examples
/// ```
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
///
/// let _scope = env::scope(vec![env::remove_var("FOO")]);
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
pub fn scope(guards: Vec<EnvVarGuard>) -> EnvScope {
    EnvScope::new(guards)
}

/// Create a scope after running the provided builder while holding the lock.
///
/// Builders must use the provided lock's methods (for example
/// `lock.set_var`/`lock.remove_var`) instead of the standalone helpers to
/// avoid re-entrant locking overhead.
///
/// # Examples
/// ```
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
///
/// let _scope = env::scope_with(|lock| vec![lock.remove_var("FOO")]);
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
pub fn scope_with<F>(builder: F) -> EnvScope
where
    F: FnOnce(&EnvVarLock) -> Vec<EnvVarGuard>,
{
    EnvScope::new_with(builder)
}

/// Create a scope from an iterator of guards.
///
/// # Examples
/// ```
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
///
/// let _scope = env::scope_from_iter([env::remove_var("FOO")]);
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
pub fn scope_from_iter<I>(guards: I) -> EnvScope
where
    I: IntoIterator<Item = EnvVarGuard>,
{
    EnvScope::from_guards(guards)
}

/// Run a closure while holding the global environment lock.
///
/// # Examples
/// ```
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
///
/// env::with_lock(|lock| {
///     let _guard = lock.set_var("KEY", "VALUE");
/// });
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
pub fn with_lock<F, R>(f: F) -> R
where
    F: FnOnce(&EnvVarLock) -> R,
{
    let lock = lock();
    f(&lock)
}
