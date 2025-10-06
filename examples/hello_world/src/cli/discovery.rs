#[cfg(all(test, unix))]
use camino::Utf8PathBuf;

use crate::error::HelloWorldError;

fn discovery() -> ortho_config::ConfigDiscovery {
    ortho_config::ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .build()
}

#[cfg(all(test, unix))]
pub(super) fn collect_config_candidates() -> Vec<Utf8PathBuf> {
    discovery().utf8_candidates()
}

pub(super) fn discover_config_figment()
-> Result<Option<ortho_config::figment::Figment>, HelloWorldError> {
    discovery().load_first().map_err(HelloWorldError::from)
}
