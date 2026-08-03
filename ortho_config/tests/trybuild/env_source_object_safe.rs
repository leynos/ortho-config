//! Trybuild fixture: a downstream `EnvSource` implementation.
//!
//! Deliberately avoids `MapEnv`, so nothing here depends on the crate's own
//! implementor. If `EnvSource` stopped being object-safe, lost a supertrait, or
//! changed `get`'s return type, this fixture would fail to compile.

use ortho_config::{ConfigDiscovery, EnvSource, SharedEnvSource};
use std::ffi::OsString;
use std::sync::Arc;

/// Answers a single variable and reports every other name as unset.
#[derive(Debug)]
struct SelectorOnlyEnv {
    key: &'static str,
    value: OsString,
}

impl EnvSource for SelectorOnlyEnv {
    fn get(&self, key: &str) -> Option<OsString> {
        (key == self.key).then(|| self.value.clone())
    }
}

/// Fails to compile unless `EnvSource` implementors satisfy the supertraits.
fn requires_debug_send_sync<T: std::fmt::Debug + Send + Sync>(value: T) -> T {
    value
}

fn main() {
    let source = requires_debug_send_sync(SelectorOnlyEnv {
        key: "DEMO_CONFIG",
        value: OsString::from("/etc/demo.toml"),
    });

    // Unsized coercion to the trait object, then the documented alias.
    let shared: Arc<dyn EnvSource> = Arc::new(source);
    let shared: SharedEnvSource = shared;

    let discovery = ConfigDiscovery::builder("demo")
        .env_var("DEMO_CONFIG")
        .env_source(shared)
        .build();

    assert!(
        discovery
            .candidates()
            .iter()
            .any(|path| path.ends_with("demo.toml")),
        "the injected selector should contribute a candidate"
    );
}
