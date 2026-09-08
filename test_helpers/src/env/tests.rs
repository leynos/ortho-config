//! Unit tests for environment helpers.

use super::*;
use anyhow::{Context, Result, ensure};
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
        let observed = match env_value(&key) {
            Ok(temporary_value) => temporary_value,
            Err(error) => panic!("worker environment value should be readable: {error:#}"),
        };
        assert_eq!(observed, value);
        drop(guard);
        let restored = match env_value(&key) {
            Ok(restored_value) => restored_value,
            Err(error) => panic!("worker original environment value should be readable: {error:#}"),
        };
        assert_eq!(restored, "original");
    }
}

fn assert_join_success(handle: thread::JoinHandle<()>) {
    if let Err(payload) = handle.join() {
        panic!("thread panicked during join: {payload:?}");
    }
}

// Centralizes environment variable lookups while retaining the original error.
fn env_value(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("read required environment variable {key}"))
}

fn setup_test_env(key: &str, value: &str) {
    super::with_lock(|_lock| {
        // SAFETY: Serialised by ENV_MUTEX held via with_lock; no concurrent env access.
        unsafe { super::env_set_var(key, OsStr::new(value)) }
    });
}

fn cleanup_test_env(key: &str) {
    super::with_lock(|_lock| {
        // SAFETY: Serialised by ENV_MUTEX held via with_lock; no concurrent env access.
        unsafe { super::env_remove_var(key) }
    });
}

fn test_guard_lifecycle<F, A>(
    key: &str,
    original: &str,
    create_guard: F,
    assert_during: A,
) -> Result<()>
where
    F: FnOnce(&str) -> EnvVarGuard,
    A: FnOnce(&str) -> Result<()>,
{
    setup_test_env(key, original);
    let during_guard = {
        let _guard = create_guard(key);
        assert_during(key)
    };
    let result = (|| {
        during_guard?;
        let restored = env_value(key)?;
        ensure!(
            restored == original,
            "environment guard must restore its original value"
        );
        Ok(())
    })();
    cleanup_test_env(key);
    result
}

#[test]
fn set_var_restores_original() -> Result<()> {
    test_guard_lifecycle(
        "TEST_HELPERS_SET_VAR",
        "orig",
        |key| set_var(key, "temp"),
        |key| {
            let temporary = env_value(key)?;
            ensure!(
                temporary == "temp",
                "environment guard must expose its temporary value"
            );
            Ok(())
        },
    )
}

#[test]
fn remove_var_restores_value() -> Result<()> {
    test_guard_lifecycle(
        "TEST_HELPERS_REMOVE_VAR",
        "to-be-removed",
        |key| remove_var(key),
        |key| {
            let removed = std::env::var(key).is_err();
            ensure!(
                removed,
                "environment guard must remove its value while held"
            );
            Ok(())
        },
    )
}

#[test]
fn set_var_unsets_when_absent() {
    let key = "TEST_HELPERS_UNSET";
    cleanup_test_env(key);
    {
        let _guard = set_var(key, "tmp");
        let temporary = env_value(key).expect("temporary environment value should be readable");
        assert_eq!(temporary, "tmp");
    }
    let unset = std::env::var(key).is_err();
    assert!(unset);
}

#[test]
fn env_value_reports_missing_variable() {
    let key = "TEST_HELPERS_MISSING";
    cleanup_test_env(key);

    let error = env_value(key).expect_err("missing environment values should be reported");
    let rendered = format!("{error:#}");

    assert!(
        rendered.contains(key),
        "missing-variable error should name its key: {rendered}"
    );
    assert!(
        !rendered.contains("{key}"),
        "missing-variable error must interpolate the key: {rendered}"
    );
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
        let original =
            env_value(&key).expect("original worker environment value should be readable");
        assert_eq!(original, "original");
        cleanup_test_env(&key);
    }

    let same_key = "TEST_HELPERS_CONCURRENT_SAME_KEY";
    setup_test_env(same_key, "base");
    let guard1 = set_var(same_key, "v1");
    let first = env_value(same_key).expect("first stacked environment value should be readable");
    assert_eq!(first, "v1");
    let guard2 = set_var(same_key, "v2");
    let second = env_value(same_key).expect("second stacked environment value should be readable");
    assert_eq!(second, "v2");
    drop(guard2);
    let restored_first =
        env_value(same_key).expect("first stacked environment value should be restored");
    assert_eq!(restored_first, "v1");
    drop(guard1);
    let original =
        env_value(same_key).expect("original stacked environment value should be restored");
    assert_eq!(original, "base");
    cleanup_test_env(same_key);
}

#[test]
fn stacking_restores_in_lifo() {
    let key = "TEST_HELPERS_STACKING";
    // Ensure clean slate.
    super::with_lock(|_lock| {
        // SAFETY: Serialised by ENV_MUTEX held via with_lock; no concurrent env access.
        unsafe { super::env_remove_var(key) }
    });
    let guard1 = set_var(key, "v1");
    let first = env_value(key).expect("first stacked environment value should be readable");
    assert_eq!(first, "v1");

    let guard2 = set_var(key, "v2");
    let second = env_value(key).expect("second stacked environment value should be readable");
    assert_eq!(second, "v2");
    drop(guard2);

    let restored_first =
        env_value(key).expect("first stacked environment value should be restored");
    assert_eq!(restored_first, "v1");
    drop(guard1);
    let unset = std::env::var(key).is_err();
    assert!(unset);
}
