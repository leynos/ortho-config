//! Test-only observation of the bytes the publish path reads from a cache
//! entry.
//!
//! The publish-path promise is that a size-mismatched entry is rejected
//! without its body being read, wherever between the open and the comparison
//! that read might be added. `CacheEntry` in the parent module routes every
//! read through here, and the count is kept per thread: `write_ir_cache`
//! completes synchronously on the calling thread, so parallel test cases
//! cannot disturb each other's observations.

use std::cell::Cell;

thread_local! {
    /// Bytes read from opened cache entries on this thread since the last
    /// reset.
    static CACHED_BODY_BYTES: Cell<usize> = const { Cell::new(0) };
}

/// Adds `bytes` to this thread's count of cache-body bytes read.
pub(super) fn record(bytes: usize) {
    CACHED_BODY_BYTES.with(|count| count.set(count.get().saturating_add(bytes)));
}

/// Clears this thread's count before a publish under test.
pub(super) fn reset() {
    CACHED_BODY_BYTES.with(|count| count.set(0));
}

/// Returns the bytes this thread has read from cache entries since the reset.
pub(super) fn observed() -> usize {
    CACHED_BODY_BYTES.with(Cell::get)
}
