//! Helpers for reading configuration files into Figment.

use crate::OrthoError;
#[cfg(feature = "yaml")]
use figment::providers::Yaml;
use figment::{
    Figment,
    providers::{Format, Toml},
};
#[cfg(feature = "json5")]
use figment_json5::Json5;

use std::path::Path;

/// Load configuration from a file, selecting the parser based on extension.
///
/// Returns `Ok(None)` if the file does not exist.
///
/// # Errors
///
/// Returns an [`OrthoError`] if reading or parsing the file fails.
#[allow(clippy::result_large_err)]
pub fn load_config_file(path: &Path) -> Result<Option<Figment>, OrthoError> {
    if !path.is_file() {
        return Ok(None);
    }
    let data = match std::fs::read_to_string(path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(OrthoError::File {
                path: path.to_path_buf(),
                source: Box::new(e),
            });
        }
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let figment = match ext.as_deref() {
        Some("json" | "json5") => {
            #[cfg(feature = "json5")]
            {
                Figment::from(Json5::string(&data))
            }
            #[cfg(not(feature = "json5"))]
            {
                return Err(OrthoError::File {
                    path: path.to_path_buf(),
                    source: Box::new(std::io::Error::other("json5 feature disabled")),
                });
            }
        }
        #[allow(clippy::unnested_or_patterns)]
        Some("yaml") | Some("yml") => {
            #[cfg(feature = "yaml")]
            {
                serde_yaml::from_str::<serde_yaml::Value>(&data).map_err(|e| OrthoError::File {
                    path: path.to_path_buf(),
                    source: Box::new(e),
                })?;
                Figment::from(Yaml::string(&data))
            }
            #[cfg(not(feature = "yaml"))]
            {
                return Err(OrthoError::File {
                    path: path.to_path_buf(),
                    source: Box::new(std::io::Error::other("yaml feature disabled")),
                });
            }
        }
        _ => {
            toml::from_str::<toml::Value>(&data).map_err(|e| OrthoError::File {
                path: path.to_path_buf(),
                source: Box::new(e),
            })?;
            Figment::from(Toml::string(&data))
        }
    };
    Ok(Some(figment))
}
