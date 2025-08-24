//! Test helpers shared across crates.
//!
//! This crate currently provides environment variable guards.
//!
//! Usage scope:
//! - Intended for test code only; do not use in production binaries or libraries.

pub mod env {
    //! Helpers for safely mutating environment variables in tests.
    //!
    //! Each mutation acquires a global mutex and returns an RAII guard that
    //! restores the previous state when dropped.
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
    use std::sync::LazyLock;

    static ENV_MUTEX: LazyLock<ReentrantMutex<()>> = LazyLock::new(ReentrantMutex::default);

    fn set_var_inner(key: &str, value: &OsStr) {
        // SAFETY: Mutations are serialised by `ENV_MUTEX`.
        unsafe { env::set_var(key, value) };
    }

    fn remove_var_inner(key: &str) {
        // SAFETY: Mutations are serialised by `ENV_MUTEX`.
        unsafe { env::remove_var(key) };
    }

    /// RAII guard restoring an environment variable to its prior value on drop.
    pub struct EnvVarGuard {
        key: String,
        original: Option<OsString>,
        _lock: ReentrantMutexGuard<'static, ()>,
    }

    /// Sets an environment variable and returns a guard restoring its prior value.
    ///
    /// # Examples
    /// ```
    /// use test_helpers::env;
    /// let _g = env::set_var("FOO", "bar");
    /// assert_eq!(std::env::var("FOO").expect("read env var"), "bar");
    /// // Dropping `_g` restores the prior value (or unsets it if none existed).
    /// ```
    pub fn set_var<K, V>(key: K, value: V) -> EnvVarGuard
    where
        K: Into<String>,
        V: AsRef<OsStr>,
    {
        let key = key.into();
        let lock = ENV_MUTEX.lock();
        let original = env::var_os(&key);
        set_var_inner(&key, value.as_ref());
        EnvVarGuard {
            key,
            original,
            _lock: lock,
        }
    }

    /// Removes an environment variable and returns a guard restoring its prior value.
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
        let key = key.into();
        let lock = ENV_MUTEX.lock();
        let original = env::var_os(&key);
        remove_var_inner(&key);
        EnvVarGuard {
            key,
            original,
            _lock: lock,
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(val) = self.original.take() {
                // guard still holds the lock
                set_var_inner(&self.key, &val);
            } else {
                remove_var_inner(&self.key);
            }
        }
    }

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

        #[test]
        fn set_var_restores_original() {
            let key = "TEST_HELPERS_SET_VAR";
            let original = "orig";
            super::with_lock(|| super::set_var_inner(key, OsStr::new(original)));
            {
                let _guard = set_var(key, "temp");
                assert_eq!(std::env::var(key).expect("read env var"), "temp");
            }
            assert_eq!(std::env::var(key).expect("read env var"), original);
            super::with_lock(|| super::remove_var_inner(key));
        }

        #[test]
        fn remove_var_restores_value() {
            let key = "TEST_HELPERS_REMOVE_VAR";
            let original = "to-be-removed";
            super::with_lock(|| super::set_var_inner(key, OsStr::new(original)));
            {
                let _guard = remove_var(key);
                assert!(std::env::var(key).is_err());
            }
            assert_eq!(std::env::var(key).expect("read env var"), original);
            super::with_lock(|| super::remove_var_inner(key));
        }

        #[test]
        fn set_var_unsets_when_absent() {
            let key = "TEST_HELPERS_UNSET";
            super::with_lock(|| super::remove_var_inner(key));
            {
                let _guard = set_var(key, "tmp");
                assert_eq!(std::env::var(key).expect("read env var"), "tmp");
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
                .map(|key| {
                    let barrier = Arc::clone(&barrier);
                    thread::spawn(move || {
                        barrier.wait();
                        let value = format!("value-{key}");
                        let _g = set_var(&key, &value);
                        assert_eq!(std::env::var(&key).expect("read env var"), value);
                    })
                })
                .collect();

            for handle in handles {
                handle.join().expect("thread to join");
            }

            for key in keys {
                assert!(std::env::var(key).is_err());
            }
        }

        #[test]
        fn stacking_restores_in_lifo() {
            let key = "TEST_HELPERS_STACKING";
            // Ensure clean slate
            super::with_lock(|| super::remove_var_inner(key));
            {
                let _g1 = set_var(key, "v1");
                assert_eq!(std::env::var(key).expect("read env var"), "v1");
                {
                    let _g2 = set_var(key, "v2");
                    assert_eq!(std::env::var(key).expect("read env var"), "v2");
                }
                assert_eq!(std::env::var(key).expect("read env var"), "v1");
            }
            assert!(std::env::var(key).is_err());
        }
    }
}
