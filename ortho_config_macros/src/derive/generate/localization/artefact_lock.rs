//! Inter-process serialisation of identifier artefact publication.
//!
//! `OUT_DIR` is scoped to the package, not to the target being compiled, so a
//! package with several targets can have several compiler processes reaching
//! one artefact directory at once. Procedural-macro side effects carry no
//! ordering or serialisation guarantee, so without this lock two writers can
//! interleave and lose entries: a writer that merged before a peer's fragment
//! landed would replace the peer's complete inventory with that stale
//! rendering.
//!
//! A unique temporary filename is not a substitute. It removes the collision
//! on the shared `.tmp` path, but it leaves the read-modify-write of the
//! merged inventory unsynchronised. The lock therefore spans the whole
//! sequence: fragment write, merge, and rendered-file publication.

use std::fs;
use std::path::Path;

use proc_macro2::Span;

/// Names the lock file created inside the artefact directory.
const LOCK_FILE: &str = "cli-identifiers.lock";

/// Holds the exclusive artefact-publication lock for its whole lifetime.
///
/// The lock is released when this guard is dropped and the underlying file
/// handle closes, which covers the caller's error paths as well as its success
/// path.
pub(super) struct LockGuard {
    /// Keeps the locked handle open; the guard owns the lock's lifetime.
    _file: fs::File,
}

/// Acquires the exclusive publication lock for one artefact directory.
///
/// Blocks until a peer process releases the lock. Returns the held guard; the
/// caller must keep it alive for the whole fragment-write, merge, and
/// publication sequence.
pub(super) fn lock(root: &Path) -> syn::Result<LockGuard> {
    let path = root.join(LOCK_FILE);
    let file = fs::File::create(&path).map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!(
                "cannot open identifier artefact lock {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
                path.display()
            ),
        )
    })?;
    file.lock().map_err(|error| {
        syn::Error::new(
            Span::call_site(),
            format!(
                "cannot lock identifier artefact directory {}: {error}; unset ORTHO_CONFIG_EMIT_IDENTIFIERS or fix permissions",
                root.display()
            ),
        )
    })?;
    Ok(LockGuard { _file: file })
}

/// Reports whether the publication lock is currently free.
///
/// Never blocks: a held lock is reported as `false` rather than waited for, so
/// callers can surface a stuck peer instead of hanging. This exists so lock
/// regression tests can observe the lock's state through the same path the
/// production code locks, rather than duplicating the filename.
#[cfg(test)]
pub(super) fn is_free(root: &Path) -> std::io::Result<bool> {
    let file = fs::File::create(root.join(LOCK_FILE))?;
    match file.try_lock() {
        Ok(()) => Ok(true),
        Err(fs::TryLockError::WouldBlock) => Ok(false),
        Err(fs::TryLockError::Error(error)) => Err(error),
    }
}
