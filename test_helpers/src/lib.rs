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
    //! Each mutation acquires a global re-entrant mutex and returns an RAII guard that:
    //! - Holds the mutex for the entire lifetime of the guard to serialise all
    //!   environment mutations across threads.
    //! - Restores the previous state when dropped (removing the variable if it
    //!   was previously absent).
    //!
    //! Behaviour:
    //! - Stacking multiple guards for the same key is supported and restores in
    //!   LIFO order.
    //! - The coarse-grained lock trades parallelism for safety; tests that mutate
    //!   the environment will be serialised while a guard is in scope.
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
        let lock = ENV_MUTEX.lock();
        let original = env::var_os(&key_string);
        mutator(&key_string);
        EnvVarGuard {
            key: key_string,
            original,
            _lock: lock,
        }
    }

    /// RAII guard restoring an environment variable to its prior value on drop.
    #[must_use = "dropping restores the prior value"]
    pub struct EnvVarGuard {
        key: String,
        original: Option<OsString>,
        _lock: ReentrantMutexGuard<'static, ()>,
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
    /// Access is serialised by a global re-entrant mutex held by the returned
    /// guard.
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
    /// Access is serialised by a global re-entrant mutex held by the returned
    /// guard.
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
            if let Some(val) = self.original.take() {
                // SAFETY: Guard still holds `ENV_MUTEX`.
                unsafe { env_set_var(&self.key, &val) };
            } else {
                // SAFETY: Guard still holds `ENV_MUTEX`.
                unsafe { env_remove_var(&self.key) };
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn with_lock<F, R>(f: F) -> R
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
        fn concurrent_mutations_are_serialised() {
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
