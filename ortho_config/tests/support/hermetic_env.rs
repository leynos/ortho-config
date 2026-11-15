//! An injected environment that no host file or directory can reach.
//!
//! Derived loading searches the working directory, the XDG bases and the home
//! directory. A closed `MapEnv` already has no `HOME`, but it still reports the
//! *process's* working directory and falls back to `/etc/xdg`, so a matching
//! file in either place would join the load. [`hermetic`] pins both to an
//! empty fixture root.

use ortho_config::MapEnv;
use std::path::Path;

/// Pin the working directory and the XDG bases of `base` to `empty_root`.
///
/// `empty_root` must hold no configuration file. A caller-set
/// `XDG_CONFIG_DIRS` is replaced, since the point is that nothing outside the
/// fixture contributes.
#[must_use]
pub fn hermetic(base: MapEnv, empty_root: &Path) -> MapEnv {
    base.with_current_dir(empty_root)
        .with_var("XDG_CONFIG_HOME", empty_root.join("xdg-home-empty"))
        .with_var("XDG_CONFIG_DIRS", empty_root.join("xdg-dirs-empty"))
}
