//! Helpers for safely mutating environment variables in tests.
//!
//! Each mutation acquires a global re-entrant mutex only for the duration of
//! the set/remove operation, returning an RAII guard that:
//! - Restores the previous state when dropped (removing the variable if it
//!   was previously absent).
//! - Re-acquires the mutex during restoration to avoid overlapping writes.
//!
//! Behaviour:
//! - Stacking multiple guards for the same key is supported and restores in
//!   LIFO order.
//! - Operations on different keys may interleave between guard creation and
//!   drop, improving parallelism while still preventing races inside each
//!   mutation operation.
//! - Overlapping guards for the same key across threads can still observe
//!   last-drop-wins semantics; avoid interleaving mutations of the same key
//!   unless you coordinate access externally.
//! - Use [`lock`] when tests need exclusive access across multiple
//!   operations that touch shared keys.
//!
//! # Examples
//!
//! ```
//! use test_helpers::env;
//!
//! let _g = env::set_var("KEY", "VALUE");
//! // `KEY` is set to `VALUE` for the duration of the guard.
//! ```

use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::sync::LazyLock;

static ENV_MUTEX: LazyLock<ReentrantMutex<()>> = LazyLock::new(ReentrantMutex::default);

/// Wrapper around `std::env::set_var`.
///
/// # Safety
///
/// Callers must ensure the global environment is synchronised.
unsafe fn env_set_var(key: &str, value: &OsStr) {
    unsafe { env::set_var(key, value) };
}

/// Wrapper around `std::env::remove_var`.
///
/// # Safety
///
/// Callers must ensure the global environment is synchronised.
unsafe fn env_remove_var(key: &str) {
    unsafe { env::remove_var(key) };
}

/// Helper function that handles the common pattern of environment variable mutation.
fn mutate_env_var<K, F>(key: K, mutator: F) -> EnvVarGuard
where
    K: Into<String>,
    F: FnOnce(&str),
{
    let key_string = key.into();
    let guard = ENV_MUTEX.lock();
    mutate_env_var_locked(key_string, mutator, &guard)
}

/// Helper for mutating environment variables when the lock is already held.
fn mutate_env_var_locked<F>(
    key: String,
    mutator: F,
    _guard: &ReentrantMutexGuard<'static, ()>,
) -> EnvVarGuard
where
    F: FnOnce(&str),
{
    let original = env::var_os(&key);
    mutator(&key);
    EnvVarGuard { key, original }
}

/// RAII guard restoring an environment variable to its prior value on drop.
#[must_use = "dropping restores the prior value"]
pub struct EnvVarGuard {
    key: String,
    original: Option<OsString>,
}

/// RAII guard that serialises environment access for its lifetime.
///
/// Use this when a test needs exclusive access to environment state across
/// multiple operations, such as coordinating shared keys across threads.
///
/// # Examples
/// ```
/// use test_helpers::env;
///
/// let _lock = env::lock();
/// let _guard = env::set_var("KEY", "VALUE");
/// // Environment mutations remain serialised while `_lock` is alive.
/// ```
#[must_use = "dropping releases the environment lock"]
pub struct EnvVarLock {
    guard: ReentrantMutexGuard<'static, ()>,
}

impl EnvVarLock {
    /// Sets an environment variable while holding the global lock.
    pub fn set_var<K, V>(&self, key: K, value: V) -> EnvVarGuard
    where
        K: Into<String>,
        V: AsRef<OsStr>,
    {
        mutate_env_var_locked(
            key.into(),
            |k| unsafe { env_set_var(k, value.as_ref()) },
            &self.guard,
        )
    }

    /// Removes an environment variable while holding the global lock.
    pub fn remove_var<K>(&self, key: K) -> EnvVarGuard
    where
        K: Into<String>,
    {
        mutate_env_var_locked(key.into(), |k| unsafe { env_remove_var(k) }, &self.guard)
    }
}

/// RAII scope that holds the environment lock while retaining guards.
///
/// This is useful when a test needs to clear or set multiple environment
/// variables and keep the lock for the duration of the test to avoid
/// interleaving with other environment mutations.
///
/// # Examples
/// ```
/// use test_helpers::env;
///
/// let guards = vec![env::remove_var("FOO"), env::remove_var("BAR")];
/// let _scope = env::EnvScope::new(guards);
/// ```
#[must_use = "dropping releases the environment lock and restores guards"]
pub struct EnvScope {
    _lock: EnvVarLock,
    guards: Vec<EnvVarGuard>,
}

impl EnvScope {
    /// Create a scope that holds the global lock and retains the guards.
    ///
    /// # Examples
    /// ```
    /// use test_helpers::env;
    ///
    /// let guards = vec![env::remove_var("FOO"), env::remove_var("BAR")];
    /// let _scope = env::EnvScope::new(guards);
    /// ```
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
    /// Builders must use the provided lock's methods (for example
    /// `lock.set_var`/`lock.remove_var`) instead of the standalone helpers to
    /// avoid re-entrant locking overhead.
    ///
    /// # Examples
    /// ```
    /// use test_helpers::env;
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
        // Ensure guard restoration happens while the environment lock is held.
        let guards = std::mem::take(&mut self.guards);
        drop(guards);
    }
}

impl fmt::Debug for EnvVarGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnvVarGuard")
            .field("key", &self.key)
            .field("had_original", &self.original.is_some())
            .finish_non_exhaustive()
    }
}

/// Sets an environment variable and returns a guard restoring its prior value.
///
/// # Safety
/// Although this function is safe to call, it mutates process-wide state.
/// Access is serialised by a global re-entrant mutex during the mutation
/// and again during restoration; other keys may interleave between those
/// operations.
///
/// # Examples
/// ```
/// use test_helpers::env;
/// let _g = env::set_var("FOO", "bar");
/// assert!(matches!(std::env::var("FOO"), Ok(ref value) if value == "bar"));
/// // Dropping `_g` restores the prior value (or unsets it if none existed).
/// ```
pub fn set_var<K, V>(key: K, value: V) -> EnvVarGuard
where
    K: Into<String>,
    V: AsRef<OsStr>,
{
    mutate_env_var(key, |k| unsafe { env_set_var(k, value.as_ref()) })
}

/// Removes an environment variable and returns a guard restoring its prior value.
///
/// # Safety
/// Although this function is safe to call, it mutates process-wide state.
/// Access is serialised by a global re-entrant mutex during the mutation
/// and again during restoration; other keys may interleave between those
/// operations.
///
/// # Examples
/// ```
/// use test_helpers::env;
/// let _g = env::remove_var("FOO");
/// assert!(std::env::var("FOO").is_err());
/// // Dropping `_g` restores the prior value (if any).
/// ```
pub fn remove_var<K>(key: K) -> EnvVarGuard
where
    K: Into<String>,
{
    mutate_env_var(key, |k| unsafe { env_remove_var(k) })
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        let _guard = ENV_MUTEX.lock();
        if let Some(val) = self.original.take() {
            // SAFETY: We hold `ENV_MUTEX` during restoration.
            unsafe { env_set_var(&self.key, &val) };
        } else {
            // SAFETY: We hold `ENV_MUTEX` during restoration.
            unsafe { env_remove_var(&self.key) };
        }
    }
}

/// Acquire the global environment lock for the lifetime of the guard.
///
/// # Examples
/// ```
/// use test_helpers::env;
///
/// let _lock = env::lock();
/// let _guard = env::set_var("KEY", "VALUE");
/// ```
pub fn lock() -> EnvVarLock {
    EnvVarLock {
        guard: ENV_MUTEX.lock(),
    }
}

/// Create a scope that holds the global lock and retains the guards.
///
/// # Examples
/// ```
/// use test_helpers::env;
///
/// let _scope = env::scope(vec![env::remove_var("FOO")]);
/// ```
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
/// use test_helpers::env;
///
/// let _scope = env::scope_with(|lock| vec![lock.remove_var("FOO")]);
/// ```
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
/// use test_helpers::env;
///
/// let _scope = env::scope_from_iter([env::remove_var("FOO")]);
/// ```
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
/// use test_helpers::env;
///
/// env::with_lock(|| {
///     let _guard = env::set_var("KEY", "VALUE");
/// });
/// ```
pub fn with_lock<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let _guard = ENV_MUTEX.lock();
    f()
}

#[cfg(test)]
mod tests;
