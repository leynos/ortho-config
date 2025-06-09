use crate::OrthoError;
#[cfg(feature = "json")]
use figment::providers::Json;
#[cfg(feature = "yaml")]
use figment::providers::Yaml;
use figment::{
    Figment,
    providers::{Format, Toml},
};
#[cfg(feature = "json")]
use serde_json;
#[cfg(feature = "yaml")]
use serde_yaml;
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
    let data = std::fs::read_to_string(path).map_err(|e| OrthoError::File {
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let figment = match ext.as_deref() {
        Some("json") => {
            #[cfg(feature = "json")]
            {
                serde_json::from_str::<serde_json::Value>(&data).map_err(|e| OrthoError::File {
                    path: path.to_path_buf(),
                    source: Box::new(e),
                })?;
                Figment::from(Json::string(&data))
            }
            #[cfg(not(feature = "json"))]
            {
                return Err(OrthoError::File {
                    path: path.to_path_buf(),
                    source: Box::new(std::io::Error::other("json feature disabled")),
                });
            }
        }
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
