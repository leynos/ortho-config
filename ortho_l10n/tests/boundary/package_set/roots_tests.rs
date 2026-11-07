//! Regression coverage for package-set cache outcomes and caller deadlines.

use super::{PackagedCrate, PackagedSetCache};
use camino::Utf8PathBuf;
use rstest::rstest;
use std::cell::Cell;
use std::time::{Duration, Instant};

#[rstest]
fn sequence_bounded_packaging_failure_does_not_poison_a_later_caller() {
    let cache = PackagedSetCache::new();
    let sequence_deadline = Instant::now() + Duration::from_secs(5);
    let mut attempted_bounded_build = false;
    let error = cache
        .prepare_with(Some(sequence_deadline), |deadline| {
            attempted_bounded_build = true;
            assert_eq!(deadline, Some(sequence_deadline));
            anyhow::bail!("packaging failed for this caller's sequence deadline")
        })
        .expect_err("the sequence-bounded build must report its failure");

    assert!(attempted_bounded_build, "the bounded build should run");
    assert!(error.to_string().contains("sequence deadline"));
    assert!(
        cache.roots.get().is_none(),
        "a failure must not cache roots"
    );
    assert!(
        cache.failure.get().is_none(),
        "a caller-specific failure must not poison the shared cache"
    );

    cache
        .prepare_with(None, |_| {
            Ok(vec![PackagedCrate {
                name: "ortho_l10n",
                version: "0.0.0",
                root: Utf8PathBuf::new(),
            }])
        })
        .expect("a later unbounded caller can retry packaging");

    assert_eq!(cache.roots.get().map(Vec::len), Some(1));
}

#[rstest]
fn unbounded_failure_is_cached_without_repeating_the_build() {
    let cache = PackagedSetCache::new();
    let attempts = Cell::new(0);
    let first = cache
        .prepare_with(None, |_| {
            attempts.set(attempts.get() + 1);
            anyhow::bail!("packaging failed")
        })
        .expect_err("the first build must fail");
    let later = cache
        .prepare_with(None, |_| {
            attempts.set(attempts.get() + 1);
            Ok(Vec::new())
        })
        .expect_err("the cached failure must be returned");

    assert_eq!(first.to_string(), "packaging failed");
    assert_eq!(later.to_string(), first.to_string());
    assert_eq!(attempts.get(), 1, "the failed build must not be repeated");
}

#[rstest]
fn bounded_caller_waiting_for_preparation_reports_sequence_deadline() {
    let cache = PackagedSetCache::new();
    let _preparation = cache.preparation.lock().expect("lock preparation");
    let attempted = Cell::new(false);
    let deadline = Instant::now() + Duration::from_millis(30);
    let error = cache
        .prepare_with(Some(deadline), |_| {
            attempted.set(true);
            Ok(Vec::new())
        })
        .expect_err("the caller must time out waiting for preparation");

    assert!(error.to_string().contains("enclosing sequence deadline"));
    assert!(
        !attempted.get(),
        "the waiting caller must not start a build"
    );
}
