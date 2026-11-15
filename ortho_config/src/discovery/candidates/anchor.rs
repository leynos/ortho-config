//! Anchoring of environment-named relative paths.
//!
//! A path an environment variable names (`XDG_CONFIG_HOME`, `HOME`, a config
//! path selector) may be relative. Left as it is, it resolves against the
//! *process* working directory when opened, which escapes a working directory
//! pinned on an injected source. Anchoring resolves it against the source's own
//! [`EnvSource::current_dir`](crate::EnvSource::current_dir) instead, so the
//! source stays the single authority on the ambient directory. For the process
//! source the two are the same directory, so nothing changes.

use std::path::PathBuf;

use super::ConfigDiscovery;

impl ConfigDiscovery {
    /// Resolve `path` against the source's working directory when it is relative.
    ///
    /// Returns `None` for a relative path when the source cannot report a
    /// working directory: such a candidate could only be read relative to the
    /// process, which is exactly what an injected source must not allow.
    pub(super) fn anchored(&self, path: PathBuf) -> Option<PathBuf> {
        // A rooted path with no drive (`/users/x` on Windows) is not
        // `is_absolute`, but it names a fixed location on whatever drive is
        // current, so it is not a path to anchor to the working directory.
        if path.is_absolute() || path.has_root() {
            return Some(path);
        }
        self.env_source
            .current_dir()
            .ok()
            .map(|directory| directory.join(path))
    }
}
