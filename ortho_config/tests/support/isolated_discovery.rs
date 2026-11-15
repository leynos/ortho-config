//! Closed discovery sources for subcommand test fixtures.

use ortho_config::MapEnv;
use std::path::Path;

/// Return a discovery source that cannot read host configuration locations.
///
/// On Unix and Redox, the XDG system directory is pinned to `root`. Other
/// platforms use a closed map source without a native directory fallback.
pub fn isolated_discovery(root: &Path) -> MapEnv {
    #[cfg(any(unix, target_os = "redox"))]
    {
        MapEnv::new().with_var("XDG_CONFIG_DIRS", root)
    }
    #[cfg(not(any(unix, target_os = "redox")))]
    {
        let _ = root;
        MapEnv::new()
    }
}
