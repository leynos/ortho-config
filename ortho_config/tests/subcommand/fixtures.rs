//! Shared fixtures for the subcommand integration suites.
//!
//! Each suite exercises a distinct configuration boundary, but all of them
//! start from the same place: a temporary file base plus a discovery source
//! closed against the ambient environment, so a developer's real `HOME` or
//! `XDG_CONFIG_HOME` can never reach an assertion.
//!
//! # Scope and re-use policy
//!
//! This module is owned by the `subcommand` integration target and is
//! reachable from its child modules as `super::fixtures`. Suites in this
//! target must obtain their discovery source from here rather than repeating
//! the `#[cfg]`-guarded construction inline, because a hand-rolled copy that
//! omits one platform branch silently reads the ambient environment on that
//! platform instead of failing.
//!
//! File contents and merge-source values stay with their tests. Those *are*
//! the behaviour under test, so a fixture that chose them would hide what each
//! case demonstrates.
//!
//! Other integration targets do not consume these fixtures. The discovery
//! suites build a [`ortho_config::ConfigDiscovery`] through `discovery_with`
//! in `ortho_config/tests/support/discovery_builder.rs` instead; that helper
//! returns a discovery over a caller-supplied environment and owns no
//! temporary root, so it does not fit the suites here.

use anyhow::{Context as _, Result};
use ortho_config::MapEnv;
use rstest::fixture;
use std::path::Path;

/// A temporary directory paired with a discovery source that cannot see the
/// ambient environment.
pub(crate) struct IsolatedRoot {
    root: tempfile::TempDir,
    discovery: MapEnv,
}

impl IsolatedRoot {
    /// Returns the directory holding this fixture's configuration files.
    #[must_use]
    pub(crate) fn path(&self) -> &Path {
        self.root.path()
    }

    /// Returns the closed discovery source, ready for extra variables.
    ///
    /// A copy is returned rather than a borrow because [`MapEnv::with_var`]
    /// consumes `self`; the copy keeps the surrounding fixture whole, so the
    /// temporary directory is still dropped exactly once, after the test.
    #[must_use]
    pub(crate) fn discovery(&self) -> MapEnv {
        self.discovery.clone()
    }
}

/// Returns a closed discovery source with the Unix global rung pinned to
/// `root`.
///
/// The global rung is pinned so the search cannot walk out of the fixture into
/// the developer's own `XDG_CONFIG_DIRS`. Everything else is empty rather than
/// ambient, so the only variables a test observes are the ones it sets.
#[cfg(any(unix, target_os = "redox"))]
pub(crate) fn close_discovery(root: &Path) -> MapEnv {
    MapEnv::new().with_var("XDG_CONFIG_DIRS", root)
}

/// Returns a closed discovery source on non-Unix and non-Redox targets.
///
/// Windows has no XDG support, so an empty source is already closed.
#[cfg(not(any(unix, target_os = "redox")))]
pub(crate) fn close_discovery(_root: &Path) -> MapEnv {
    MapEnv::new()
}

/// Returns a temporary root whose discovery source cannot see the environment.
///
/// The result is fallible so a suite reports a failed fixture setup as a test
/// failure rather than a panic; `rstest` passes the `Result` through and the
/// test decides whether to propagate it.
#[fixture]
pub(crate) fn isolated_root() -> Result<IsolatedRoot> {
    let root = tempfile::tempdir().context("create isolated discovery root")?;
    let discovery = close_discovery(root.path());
    Ok(IsolatedRoot { root, discovery })
}
