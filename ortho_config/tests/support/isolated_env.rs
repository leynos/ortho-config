//! Environment overrides that pin a derivation run's XDG bases to a fixture.
//!
//! Derived loading builds its own `ConfigDiscovery` from the injected
//! environment source. Two of the rungs that discovery walks reach outside
//! it, and the derive macro offers no attribute that redirects either:
//!
//! - The project rung resolves its root from the injected source's working
//!   directory (`EnvSource::current_dir`). An unpinned source reports the
//!   process's directory, so a stray `.config.toml` beside the test binary
//!   would be loaded. `support/hermetic_env.rs` pins the directory to an empty
//!   fixture root, which closes this rung.
//! - The XDG rung falls back to the platform default `/etc/xdg` whenever the
//!   injected source has no `XDG_CONFIG_DIRS`, so a system-wide file could be
//!   discovered ahead of a fixture. [`with_host_overrides`] closes this one:
//!   pinning the bases to the fixture root ranks them above the default while
//!   leaving any `xdg/<app>/config.toml` the fixture deliberately writes
//!   reachable.

use ortho_config::{EnvSource as _, MapEnv};
use std::path::Path;

/// Point the XDG search bases at `fixture_root` unless the caller set them.
///
/// A caller that pins its own `XDG_CONFIG_DIRS` keeps that value, because a
/// suite testing XDG resolution needs to choose the bases deliberately.
///
/// # Examples
///
/// ```rust,ignore
/// use ortho_config::MapEnv;
///
/// let temp_dir = tempfile::tempdir()?;
/// let env = with_host_overrides(MapEnv::new(), temp_dir.path());
/// # Ok::<(), anyhow::Error>(())
/// ```
#[must_use]
pub fn with_host_overrides(base: MapEnv, fixture_root: &Path) -> MapEnv {
    if base.get("XDG_CONFIG_DIRS").is_some() {
        return base;
    }
    base.with_var("XDG_CONFIG_DIRS", fixture_root.as_os_str())
}
