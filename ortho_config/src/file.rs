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

#[expect(clippy::result_large_err, reason = "propagating parsing errors")]
fn wrap_parse<T, E>(path: &Path, res: Result<T, E>) -> Result<T, OrthoError>
where
    E: std::error::Error + Send + Sync + 'static,
{
    res.map_err(|e| OrthoError::File {
        path: path.to_path_buf(),
        source: Box::new(e),
    })
}

#[cfg(any(not(feature = "json5"), not(feature = "yaml")))]
fn feature_disabled(path: &Path, feature: &str) -> OrthoError {
    OrthoError::File {
        path: path.to_path_buf(),
        source: Box::new(std::io::Error::other(format!("{feature} feature disabled"))),
    }
}

#[cfg(feature = "json5")]
#[expect(clippy::result_large_err, reason = "propagating parsing errors")]
fn parse_json(path: &Path, data: &str) -> Result<Figment, OrthoError> {
    wrap_parse(path, json5::from_str::<serde_json::Value>(data))?;
    Ok(Figment::from(Json5::string(data)))
}

#[cfg(not(feature = "json5"))]
#[expect(clippy::result_large_err, reason = "feature-gated parsing")]
fn parse_json(path: &Path, _data: &str) -> Result<Figment, OrthoError> {
    Err(feature_disabled(path, "json5"))
}

#[cfg(feature = "yaml")]
#[expect(clippy::result_large_err, reason = "propagating parsing errors")]
fn parse_yaml(path: &Path, data: &str) -> Result<Figment, OrthoError> {
    wrap_parse(path, serde_yaml::from_str::<serde_yaml::Value>(data))?;
    Ok(Figment::from(Yaml::string(data)))
}

#[cfg(not(feature = "yaml"))]
#[expect(clippy::result_large_err, reason = "feature-gated parsing")]
fn parse_yaml(path: &Path, _data: &str) -> Result<Figment, OrthoError> {
    Err(feature_disabled(path, "yaml"))
}

#[expect(clippy::result_large_err, reason = "propagating parsing errors")]
fn parse_toml(path: &Path, data: &str) -> Result<Figment, OrthoError> {
    wrap_parse(path, toml::from_str::<toml::Value>(data))?;
    Ok(Figment::from(Toml::string(data)))
}

/// Parse configuration data according to the file extension.
///
/// Supported formats are JSON5, YAML and TOML. The `json5` and `yaml`
/// features must be enabled for those formats to be parsed.
///
/// # Errors
///
/// Returns an [`OrthoError`] if the file contents fail to parse or if the
/// required feature is disabled.
#[expect(
    clippy::result_large_err,
    reason = "Error type is library specific and intentionally large"
)]
fn parse_config_by_format(path: &Path, data: &str) -> Result<Figment, OrthoError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("json" | "json5") => parse_json(path, data),
        #[allow(clippy::unnested_or_patterns)]
        Some("yaml") | Some("yml") => parse_yaml(path, data),
        _ => parse_toml(path, data),
    }
}

/// Apply inheritance using the `extends` key.
///
/// The referenced file is loaded first and the current [`Figment`] is merged
/// over it. Cycles are detected using `visited`.
///
/// # Errors
///
/// Returns an [`OrthoError`] if the extended file fails to load or the `extends`
/// key is malformed.
#[expect(clippy::result_large_err, reason = "propagating file loading errors")]
fn process_extends(
    mut figment: Figment,
    current_path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<Figment, OrthoError> {
    match figment.find_value("extends") {
        Ok(val) => {
            let base = val.as_str().ok_or_else(|| OrthoError::File {
                path: current_path.to_path_buf(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "'extends' key must be a string",
                )),
            })?;

            let parent = current_path.parent().ok_or_else(|| OrthoError::File {
                path: current_path.to_path_buf(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Cannot determine parent directory for config file when resolving 'extends'",
                )),
            })?;

            let base_path = if Path::new(base).is_absolute() {
                PathBuf::from(base)
            } else {
                parent.join(base)
            };

            let canonical = std::fs::canonicalize(&base_path).map_err(|e| OrthoError::File {
                path: base_path.clone(),
                source: Box::new(e),
            })?;

            if let Some(base_fig) = load_config_file_inner(&canonical, visited, stack)? {
                figment = base_fig.merge(figment);
            }
            Ok(figment)
        }
        Err(e) if e.missing() => Ok(figment),
        Err(e) => Err(OrthoError::File {
            path: current_path.to_path_buf(),
            source: Box::new(e),
        }),
    }
}

/// Load configuration from a file, selecting the parser based on extension.
///
/// Returns `Ok(None)` if the file does not exist.
///
/// # Errors
///
/// Returns an [`OrthoError`] if reading or parsing the file fails.
#[expect(
    clippy::result_large_err,
    reason = "Error type is large but returned directly"
)]
pub fn load_config_file(path: &Path) -> Result<Option<Figment>, OrthoError> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    load_config_file_inner(path, &mut visited, &mut stack)
}

#[expect(clippy::result_large_err, reason = "propagating parsing and IO errors")]
fn load_config_file_inner(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<Option<Figment>, OrthoError> {
    if !path.is_file() {
        return Ok(None);
    }
    let canonical = std::fs::canonicalize(path).map_err(|e| OrthoError::File {
        path: path.to_path_buf(),
        source: Box::new(e),
    })?;
    if !visited.insert(canonical.clone()) {
        let mut cycle: Vec<String> = stack.iter().map(|p| p.display().to_string()).collect();
        cycle.push(canonical.display().to_string());
        return Err(OrthoError::CyclicExtends {
            cycle: cycle.join(" -> "),
        });
    }
    stack.push(canonical.clone());
    let result = (|| {
        let data = std::fs::read_to_string(&canonical).map_err(|e| OrthoError::File {
            path: canonical.clone(),
            source: Box::new(e),
        })?;
        let figment = parse_config_by_format(&canonical, &data)?;
        process_extends(figment, &canonical, visited, stack)
    })();
    visited.remove(&canonical);
    stack.pop();
    result.map(Some)
}
