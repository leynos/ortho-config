//! Helpers for safely mutating environment variables in tests.
//!
//! **Deprecated since 0.11.0.** Changing the process environment forces every
//! test in the binary to serialize behind a lock, which leaves runner cores
//! idle, and Rust 2024 makes `set_var` unsafe. Inject the environment instead:
//! build an `ortho_config::MapEnv` and call the derive-generated
//! `load_from_iter_with_env` family, which reads nothing from the process. A
//! test that must exercise a process-backed code path runs it in a child
//! process with `Command::env`. The v0.11.0 migration guide in the
//! `ortho-config` repository (`docs/v0-11-0-migration-guide.md`) walks through
//! both forms. The helpers remain for downstream crates that have not migrated
//! yet, and will be removed in a later release.
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
//! # #![expect(deprecated)]
//! use ortho_config_test_helpers::env;
//!
//! let _g = env::set_var("KEY", "VALUE");
//! // `KEY` is set to `VALUE` for the duration of the guard.
//! ```

#![expect(
    deprecated,
    reason = "this module implements the deprecated API and uses it internally"
)]

use parking_lot::{ReentrantMutex, ReentrantMutexGuard};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::sync::LazyLock;

/// Serializes each process environment mutation and its restoration.
static ENV_MUTEX: LazyLock<ReentrantMutex<()>> = LazyLock::new(ReentrantMutex::default);

/// Wrapper around `std::env::set_var`.
///
/// # Safety
///
/// Callers must ensure the global environment is synchronized.
#[expect(
    clippy::disallowed_methods,
    reason = "the deprecated guard API exists to mutate the process environment under a lock"
)]
unsafe fn env_set_var(key: &str, value: &OsStr) {
    unsafe { env::set_var(key, value) };
}

/// Wrapper around `std::env::remove_var`.
///
/// # Safety
///
/// Callers must ensure the global environment is synchronized.
#[expect(
    clippy::disallowed_methods,
    reason = "the deprecated guard API exists to mutate the process environment under a lock"
)]
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
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
#[must_use = "dropping restores the prior value"]
pub struct EnvVarGuard {
    /// Variable restored when this guard is dropped.
    key: String,
    /// Prior value, or `None` when the variable was originally unset.
    original: Option<OsString>,
}

/// RAII guard that serializes environment access for its lifetime.
///
/// Use this when a test needs exclusive access to environment state across
/// multiple operations, such as coordinating shared keys across threads.
///
/// # Examples
/// ```
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
///
/// let _lock = env::lock();
/// let _guard = _lock.set_var("KEY", "VALUE");
/// // Environment mutations remain serialized while `_lock` is alive.
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
#[must_use = "dropping releases the environment lock"]
pub struct EnvVarLock {
    /// Exclusive process-wide access retained for this lock's lifetime.
    guard: ReentrantMutexGuard<'static, ()>,
}

impl EnvVarLock {
    /// Internal helper for environment mutations using the held lock.
    fn mutate_with<F>(&self, key: String, operation: F) -> EnvVarGuard
    where
        F: FnOnce(&str),
    {
        mutate_env_var_locked(key, operation, &self.guard)
    }

    /// Sets an environment variable while holding the global lock.
    pub fn set_var<K, V>(&self, key: K, value: V) -> EnvVarGuard
    where
        K: Into<String>,
        V: AsRef<OsStr>,
    {
        self.mutate_with(key.into(), |k| {
            // SAFETY: The caller holds the global env lock for this mutation.
            unsafe { env_set_var(k, value.as_ref()) }
        })
    }

    /// Removes an environment variable while holding the global lock.
    pub fn remove_var<K>(&self, key: K) -> EnvVarGuard
    where
        K: Into<String>,
    {
        self.mutate_with(key.into(), |k| {
            // SAFETY: The caller holds the global env lock for this mutation.
            unsafe { env_remove_var(k) }
        })
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
/// Access is serialized by a global re-entrant mutex during the mutation
/// and again during restoration; other keys may interleave between those
/// operations.
///
/// # Examples
/// ```
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
/// let _g = env::set_var("FOO", "bar");
/// assert!(matches!(std::env::var("FOO"), Ok(ref value) if value == "bar"));
/// // Dropping `_g` restores the prior value (or unsets it if none existed).
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
pub fn set_var<K, V>(key: K, value: V) -> EnvVarGuard
where
    K: Into<String>,
    V: AsRef<OsStr>,
{
    mutate_env_var(key, |k| {
        // SAFETY: mutate_env_var holds the global env lock for this mutation.
        unsafe { env_set_var(k, value.as_ref()) }
    })
}

/// Removes an environment variable and returns a guard restoring its prior value.
///
/// # Safety
/// Although this function is safe to call, it mutates process-wide state.
/// Access is serialized by a global re-entrant mutex during the mutation
/// and again during restoration; other keys may interleave between those
/// operations.
///
/// # Examples
/// ```
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
/// let _g = env::remove_var("FOO");
/// assert!(std::env::var("FOO").is_err());
/// // Dropping `_g` restores the prior value (if any).
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
pub fn remove_var<K>(key: K) -> EnvVarGuard
where
    K: Into<String>,
{
    mutate_env_var(key, |k| {
        // SAFETY: mutate_env_var holds the global env lock for this mutation.
        unsafe { env_remove_var(k) }
    })
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
/// # #![expect(deprecated)]
/// use ortho_config_test_helpers::env;
///
/// let _lock = env::lock();
/// let _guard = env::set_var("KEY", "VALUE");
/// ```
#[deprecated(
    since = "0.11.0",
    note = "inject an `ortho_config::MapEnv` instead; see the v0.11.0 migration guide"
)]
pub fn lock() -> EnvVarLock {
    EnvVarLock {
        guard: ENV_MUTEX.lock(),
    }
}

mod scoped;
pub use scoped::{EnvScope, scope, scope_from_iter, scope_with, with_lock};

#[cfg(test)]
mod tests;
