//! Unit tests for environment helpers.

use super::*;
use std::ffi::OsStr;
use std::sync::{Arc, Barrier};
use std::thread;

fn spawn_env_worker(
    barrier: &Arc<Barrier>,
    key: String,
    iterations: usize,
) -> thread::JoinHandle<()> {
    let barrier_wait = Arc::clone(barrier);
    thread::spawn(move || run_env_worker(barrier_wait, key, iterations))
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "thread closure requires owned Arc and String to satisfy 'static"
)]
fn run_env_worker(barrier: Arc<Barrier>, key: String, iterations: usize) {
    barrier.wait();
    for iter in 0..iterations {
        let value = format!("value-{key}-{iter}");
        let guard = set_var(&key, &value);
        assert_eq!(env_value(&key), value);
        drop(guard);
        assert_eq!(env_value(&key), "original");
    }
}

fn assert_join_success(handle: thread::JoinHandle<()>) {
    handle.join().expect("thread panicked during join");
}

// Centralizes environment variable lookups for the tests; panics on
// missing/invalid values so failures are loud and easy to diagnose.
fn env_value(key: &str) -> String {
    match std::env::var(key) {
        Ok(value) => value,
        Err(err) => panic!("expected environment variable {key}: {err}"),
    }
}

fn setup_test_env(key: &str, value: &str) {
    super::with_lock(|| {
        // SAFETY: Serialised by ENV_MUTEX held via with_lock; no concurrent env access.
        unsafe { super::env_set_var(key, OsStr::new(value)) }
    });
}

fn cleanup_test_env(key: &str) {
    super::with_lock(|| {
        // SAFETY: Serialised by ENV_MUTEX held via with_lock; no concurrent env access.
        unsafe { super::env_remove_var(key) }
    });
}

fn test_guard_lifecycle<F, A>(key: &str, original: &str, create_guard: F, assert_during: A)
where
    F: FnOnce(&str) -> EnvVarGuard,
    A: FnOnce(&str),
{
    setup_test_env(key, original);
    {
        let _guard = create_guard(key);
        assert_during(key);
    }
    assert_eq!(env_value(key), original);
    cleanup_test_env(key);
}

#[test]
fn set_var_restores_original() {
    test_guard_lifecycle(
        "TEST_HELPERS_SET_VAR",
        "orig",
        |key| set_var(key, "temp"),
        |key| assert_eq!(env_value(key), "temp"),
    );
}

#[test]
fn remove_var_restores_value() {
    test_guard_lifecycle(
        "TEST_HELPERS_REMOVE_VAR",
        "to-be-removed",
        |key| remove_var(key),
        |key| assert!(std::env::var(key).is_err()),
    );
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
    const ITERATIONS: usize = 8;
    let keys: Vec<_> = (0..THREADS)
        .map(|i| format!("TEST_HELPERS_CONCURRENT_{i}"))
        .collect();
    let barrier = Arc::new(Barrier::new(THREADS));

    for key in &keys {
        setup_test_env(key, "original");
    }

    let handles: Vec<_> = keys
        .iter()
        .cloned()
        .map(|key| spawn_env_worker(&barrier, key, ITERATIONS))
        .collect();

    handles.into_iter().for_each(assert_join_success);

    for key in keys {
        assert_eq!(env_value(&key), "original");
        cleanup_test_env(&key);
    }

    let same_key = "TEST_HELPERS_CONCURRENT_SAME_KEY";
    setup_test_env(same_key, "base");
    let guard1 = set_var(same_key, "v1");
    assert_eq!(env_value(same_key), "v1");
    let guard2 = set_var(same_key, "v2");
    assert_eq!(env_value(same_key), "v2");
    drop(guard2);
    assert_eq!(env_value(same_key), "v1");
    drop(guard1);
    assert_eq!(env_value(same_key), "base");
    cleanup_test_env(same_key);
}

#[test]
fn stacking_restores_in_lifo() {
    let key = "TEST_HELPERS_STACKING";
    // Ensure clean slate.
    super::with_lock(|| {
        // SAFETY: Serialised by ENV_MUTEX held via with_lock; no concurrent env access.
        unsafe { super::env_remove_var(key) }
    });
    let guard1 = set_var(key, "v1");
    assert_eq!(env_value(key), "v1");

    let guard2 = set_var(key, "v2");
    assert_eq!(env_value(key), "v2");
    drop(guard2);

    assert_eq!(env_value(key), "v1");
    drop(guard1);
    assert!(std::env::var(key).is_err());
}
