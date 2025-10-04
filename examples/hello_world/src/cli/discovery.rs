#[cfg(test)]
use camino::Utf8PathBuf;

use crate::error::HelloWorldError;

fn discovery() -> ortho_config::ConfigDiscovery {
    ortho_config::ConfigDiscovery::builder("hello_world")
        .env_var("HELLO_WORLD_CONFIG_PATH")
        .build()
}

#[cfg(test)]
fn convert_candidates(paths: Vec<std::path::PathBuf>) -> Vec<Utf8PathBuf> {
    paths
        .into_iter()
        .filter_map(|path| Utf8PathBuf::from_path_buf(path).ok())
        .collect()
}

#[cfg(test)]
pub(super) fn collect_config_candidates() -> Vec<Utf8PathBuf> {
    convert_candidates(discovery().candidates())
}

pub(super) fn discover_config_figment()
-> Result<Option<ortho_config::figment::Figment>, HelloWorldError> {
    discovery().load_first().map_err(HelloWorldError::from)
}
