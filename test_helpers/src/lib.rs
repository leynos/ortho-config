//! Test helpers shared across crates.
//!
//! This crate currently provides environment variable guards.
//!
//! Usage scope:
//! - Intended for test code only; do not use in production binaries or libraries.

pub mod figment;
pub mod env {
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

    /// Helper function that handles the common pattern of environment variable mutation
    fn mutate_env_var<K, F>(key: K, mutator: F) -> EnvVarGuard
    where
        K: Into<String>,
        F: FnOnce(&str),
    {
        let key_string = key.into();
        let original = {
            let _guard = ENV_MUTEX.lock();
            let original = env::var_os(&key_string);
            mutator(&key_string);
            original
        };
        EnvVarGuard {
            key: key_string,
            original,
        }
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
        _guard: ReentrantMutexGuard<'static, ()>,
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
        /// # Examples
        /// ```
        /// use test_helpers::env;
        ///
        /// let _scope = env::EnvScope::new_with(|| {
        ///     vec![env::remove_var("FOO"), env::remove_var("BAR")]
        /// });
        /// ```
        pub fn new_with<F>(builder: F) -> Self
        where
            F: FnOnce() -> Vec<EnvVarGuard>,
        {
            let lock = lock();
            let guards = builder();
            Self {
                _lock: lock,
                guards,
            }
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
            _guard: ENV_MUTEX.lock(),
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
    /// # Examples
    /// ```
    /// use test_helpers::env;
    ///
    /// let _scope = env::scope_with(|| vec![env::remove_var("FOO")]);
    /// ```
    pub fn scope_with<F>(builder: F) -> EnvScope
    where
        F: FnOnce() -> Vec<EnvVarGuard>,
    {
        EnvScope::new_with(builder)
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
    mod tests {
        use super::*;
        use std::ffi::OsStr;
        use std::sync::{Arc, Barrier};
        use std::thread;

        fn spawn_env_worker(barrier: &Arc<Barrier>, key: String) -> thread::JoinHandle<()> {
            let barrier_wait = Arc::clone(barrier);
            thread::spawn(move || run_env_worker(barrier_wait, key))
        }

        #[expect(
            clippy::needless_pass_by_value,
            reason = "thread closure requires owned Arc and String to satisfy 'static"
        )]
        fn run_env_worker(barrier: Arc<Barrier>, key: String) {
            barrier.wait();
            let value = format!("value-{key}");
            let guard = set_var(&key, &value);
            assert_eq!(env_value(&key), value);
            drop(guard);
        }

        fn assert_join_success(handle: thread::JoinHandle<()>) {
            if let Err(err) = handle.join() {
                panic!("thread panicked: {err:?}");
            }
        }

        // Centralises environment variable lookups for the tests; panics on
        // missing/invalid values so failures are loud and easy to diagnose.
        fn env_value(key: &str) -> String {
            match std::env::var(key) {
                Ok(value) => value,
                Err(err) => panic!("expected environment variable {key}: {err}"),
            }
        }

        fn setup_test_env(key: &str, value: &str) {
            super::with_lock(|| unsafe { super::env_set_var(key, OsStr::new(value)) });
        }

        fn cleanup_test_env(key: &str) {
            super::with_lock(|| unsafe { super::env_remove_var(key) });
        }

        #[test]
        fn set_var_restores_original() {
            let key = "TEST_HELPERS_SET_VAR";
            let original = "orig";
            setup_test_env(key, original);
            {
                let _guard = set_var(key, "temp");
                assert_eq!(env_value(key), "temp");
            }
            assert_eq!(env_value(key), original);
            cleanup_test_env(key);
        }

        #[test]
        fn remove_var_restores_value() {
            let key = "TEST_HELPERS_REMOVE_VAR";
            let original = "to-be-removed";
            setup_test_env(key, original);
            {
                let _guard = remove_var(key);
                assert!(std::env::var(key).is_err());
            }
            assert_eq!(env_value(key), original);
            cleanup_test_env(key);
        }

        #[test]
        fn set_var_unsets_when_absent() {
            let key = "TEST_HELPERS_UNSET";
            cleanup_test_env(key);
            {
                let _guard = set_var(key, "tmp");
                assert_eq!(env_value(key), "tmp");
            }
            assert!(std::env::var(key).is_err());
        }

        #[test]
        fn concurrent_mutations_restore_values() {
            const THREADS: usize = 4;
            let keys: Vec<_> = (0..THREADS)
                .map(|i| format!("TEST_HELPERS_CONCURRENT_{i}"))
                .collect();
            let barrier = Arc::new(Barrier::new(THREADS));

            let handles: Vec<_> = keys
                .iter()
                .cloned()
                .map(|key| spawn_env_worker(&barrier, key))
                .collect();

            handles.into_iter().for_each(assert_join_success);

            for key in keys {
                assert!(std::env::var(key).is_err());
            }
        }

        #[test]
        fn stacking_restores_in_lifo() {
            let key = "TEST_HELPERS_STACKING";
            // Ensure clean slate
            super::with_lock(|| unsafe { super::env_remove_var(key) });
            let guard1 = set_var(key, "v1");
            assert_eq!(env_value(key), "v1");

            let guard2 = set_var(key, "v2");
            assert_eq!(env_value(key), "v2");
            drop(guard2);

            assert_eq!(env_value(key), "v1");
            drop(guard1);
            assert!(std::env::var(key).is_err());
        }
    }
}
