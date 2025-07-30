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

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Parse configuration data according to the file extension.
///
/// Supported formats are JSON5, YAML and TOML. The `json5` and `yaml`
/// features must be enabled for those formats to be parsed.
///
/// # Errors
///
/// Returns an [`OrthoError`] if the file contents fail to parse or if the
/// required feature is disabled.
#[allow(clippy::result_large_err)]
fn parse_config_by_format(path: &Path, data: &str) -> Result<Figment, OrthoError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);

    let figment = match ext.as_deref() {
        Some("json" | "json5") => {
            #[cfg(feature = "json5")]
            {
                Figment::from(Json5::string(data))
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
                serde_yaml::from_str::<serde_yaml::Value>(data).map_err(|e| OrthoError::File {
                    path: path.to_path_buf(),
                    source: Box::new(e),
                })?;
                Figment::from(Yaml::string(data))
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
            toml::from_str::<toml::Value>(data).map_err(|e| OrthoError::File {
                path: path.to_path_buf(),
                source: Box::new(e),
            })?;
            Figment::from(Toml::string(data))
        }
    };

    Ok(figment)
}

/// Apply inheritance using the `extends` key.
///
/// The referenced file is loaded first and the current [`Figment`] is merged
/// over it. Cycles are detected using `visited`.
///
/// # Errors
///
/// Returns an [`OrthoError`] if the extended file fails to load.
#[allow(clippy::result_large_err)]
fn process_extends(
    mut figment: Figment,
    current_path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<Figment, OrthoError> {
    if let Ok(base) = figment.extract_inner::<String>("extends") {
        let base_path = if Path::new(&base).is_absolute() {
            PathBuf::from(base)
        } else {
            current_path.parent().unwrap_or(Path::new(".")).join(base)
        };

        if let Some(base_fig) = load_config_file_inner(&base_path, visited)? {
            figment = base_fig.merge(figment);
        }
    }

    Ok(figment)
}

/// Load configuration from a file, selecting the parser based on extension.
///
/// Returns `Ok(None)` if the file does not exist.
///
/// # Errors
///
/// Returns an [`OrthoError`] if reading or parsing the file fails.
#[allow(clippy::result_large_err)]
pub fn load_config_file(path: &Path) -> Result<Option<Figment>, OrthoError> {
    let mut visited = HashSet::new();
    load_config_file_inner(path, &mut visited)
}

#[allow(clippy::result_large_err)]
fn load_config_file_inner(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<Option<Figment>, OrthoError> {
    if !path.is_file() {
        return Ok(None);
    }
    let canonical = std::fs::canonicalize(path).map_err(|e| OrthoError::File {
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;
    if !visited.insert(canonical.clone()) {
        return Err(OrthoError::File {
            path: canonical,
            source: Box::new(std::io::Error::other("cyclic extends detected")),
        });
    }
    let data = std::fs::read_to_string(&canonical).map_err(|e| OrthoError::File {
        path: canonical.clone(),
        source: Box::new(e),
    })?;
    let figment = parse_config_by_format(&canonical, &data)?;
    let figment = process_extends(figment, &canonical, visited)?;

    visited.remove(&canonical);
    Ok(Some(figment))
}
