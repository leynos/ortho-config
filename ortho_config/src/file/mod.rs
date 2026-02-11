//! Helpers for reading configuration files into Figment.

mod error;
mod extends;
mod loader;
mod parser;
mod path;
#[cfg(feature = "yaml")]
mod yaml;

pub use loader::{FileLayerChain, load_config_file, load_config_file_as_chain};
pub use path::canonicalise;
#[cfg(feature = "yaml")]
pub use yaml::SaphyrYaml;

#[cfg(test)]
mod tests;
