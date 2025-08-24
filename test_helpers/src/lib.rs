//! Test helpers shared across crates.
//!
//! This crate currently provides environment variable guards.

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

    use std::env;
    use std::ffi::{OsStr, OsString};
    use std::sync::{LazyLock, Mutex};

    static ENV_MUTEX: LazyLock<Mutex<()>> = LazyLock::new(Mutex::default);

    /// RAII guard restoring an environment variable to its prior value on drop.
    pub struct EnvVarGuard {
        key: String,
        original: Option<OsString>,
    }

    /// Sets an environment variable and returns a guard restoring its prior value.
    pub fn set_var<K, V>(key: K, value: V) -> EnvVarGuard
    where
        K: Into<String>,
        V: AsRef<OsStr>,
    {
        let key = key.into();
        let original = with_lock(|| env::var_os(&key));
        with_lock(|| unsafe { env::set_var(&key, value) });
        EnvVarGuard { key, original }
    }

    /// Removes an environment variable and returns a guard restoring its prior value.
    pub fn remove_var<K>(key: K) -> EnvVarGuard
    where
        K: Into<String>,
    {
        let key = key.into();
        let original = with_lock(|| env::var_os(&key));
        with_lock(|| unsafe { env::remove_var(&key) });
        EnvVarGuard { key, original }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(val) = self.original.take() {
                with_lock(|| unsafe { env::set_var(&self.key, val) });
            } else {
                with_lock(|| unsafe { env::remove_var(&self.key) });
            }
        }
    }

    fn with_lock<F, R>(f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let _guard = ENV_MUTEX.lock().expect("lock env mutex");
        f()
    }
}
