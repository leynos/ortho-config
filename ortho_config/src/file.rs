//! Helpers for reading configuration files into Figment.
#![expect(
    clippy::result_large_err,
    reason = "OrthoError is intentionally large throughout this module"
)]

// FIXME: Reduce OrthoError size (e.g., via Arc) to remove this expectation.
#![expect(
    clippy::result_large_err,
    reason = "Surface detailed file errors to callers"
)]

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
use std::error::Error;
use std::path::{Path, PathBuf};

/// Construct an [`OrthoError::File`] for a configuration path.
fn file_error(path: &Path, err: impl Into<Box<dyn Error + Send + Sync>>) -> OrthoError {
    OrthoError::File {
        path: path.to_path_buf(),
        source: err.into(),
    }
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
                return Err(file_error(
                    path,
                    std::io::Error::other(
                        "json5 feature disabled: enable the 'json5' feature to support this file format",
                    ),
                ));
            }
        }
        Some("yaml" | "yml") => {
            #[cfg(feature = "yaml")]
            {
                serde_yaml::from_str::<serde_yaml::Value>(data).map_err(|e| file_error(path, e))?;
                Figment::from(Yaml::string(data))
            }
            #[cfg(not(feature = "yaml"))]
            {
                return Err(file_error(
                    path,
                    std::io::Error::other("yaml feature disabled"),
                ));
            }
        }
        _ => {
            toml::from_str::<toml::Value>(data).map_err(|e| file_error(path, e))?;
            Figment::from(Toml::string(data))
        }
    };

    Ok(figment)
}

/// Validate and extract the `extends` value from `figment`.
///
/// Returns `Ok(None)` if the key is absent.
///
/// # Examples
///
/// ```rust,ignore
/// # use figment::{Figment, providers::{Format, Toml}};
/// # use std::path::{Path, PathBuf};
/// # use ortho_config::file::get_extends;
/// let figment = Figment::from(Toml::string("extends = \"base.toml\""));
/// let extends = get_extends(&figment, Path::new("cfg.toml")).unwrap();
/// assert_eq!(extends, Some(PathBuf::from("base.toml")));
/// ```
fn get_extends(figment: &Figment, current_path: &Path) -> Result<Option<PathBuf>, OrthoError> {
    match figment.find_value("extends") {
        Ok(val) => {
            let base = val.as_str().ok_or_else(|| {
                let actual_type = match &val {
                    figment::value::Value::String(..) => "string",
                    figment::value::Value::Char(..) => "char",
                    figment::value::Value::Bool(..) => "bool",
                    figment::value::Value::Num(..) => "number",
                    figment::value::Value::Empty(..) => "null",
                    figment::value::Value::Dict(..) => "object",
                    figment::value::Value::Array(..) => "array",
                };
                file_error(
                    current_path,
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("'extends' key must be a string, but found type: {actual_type}"),
                    ),
                )
            })?;
            Ok(Some(PathBuf::from(base)))
        }
        Err(e) if e.missing() => Ok(None),
        Err(e) => Err(file_error(current_path, e)),
    }
}

/// Resolve an `extends` path relative to the current file.
///
/// If `base` is relative it is joined with the parent directory of
/// `current_path` and canonicalised. Absolute paths are canonicalised
/// directly.
///
/// Canonicalisation ensures consistent absolute paths for robust cycle
/// detection and de-duplication across symlinks. On Windows this uses
/// [`dunce::canonicalize`] to avoid introducing UNC prefixes in diagnostic
/// messages.
///
/// The target must already exist as a regular file; non-existent paths
/// cause canonicalisation to fail and an error to be returned.
///
/// # Errors
///
/// Returns an [`OrthoError`] if the parent directory cannot be determined
/// or if canonicalisation fails.
///
/// # Examples
///
/// ```rust,no_run
/// # use std::path::{Path, PathBuf};
/// # use ortho_config::file::resolve_base_path;
/// # fn run() -> Result<(), ortho_config::OrthoError> {
/// let current = Path::new("/tmp/config.toml");
/// let base = PathBuf::from("base.toml");
/// let canonical = resolve_base_path(current, base)?;
/// assert!(canonical.ends_with("base.toml"));
/// # Ok(())
/// # }
/// ```
fn resolve_base_path(current_path: &Path, base: PathBuf) -> Result<PathBuf, OrthoError> {
    let parent = current_path.parent().ok_or_else(|| {
        file_error(
            current_path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot determine parent directory for config file when resolving 'extends'",
            ),
        )
    })?;
    let base = if base.is_absolute() { base } else { parent.join(base) };
    #[cfg(windows)]
    let canonical = dunce::canonicalize(&base).map_err(|e| file_error(&base, e))?;
    #[cfg(not(windows))]
    let canonical = std::fs::canonicalize(&base).map_err(|e| file_error(&base, e))?;
    Ok(canonical)
}

/// Merge `figment` over its parent configuration.
///
/// The parent is used as the base configuration with `figment` overriding its
/// values.
///
/// # Examples
///
/// ```rust
/// use figment::{Figment, providers::Toml};
/// use ortho_config::file::merge_parent;
///
/// let parent = Figment::from(Toml::string("foo = \"parent\""));
/// let child = Figment::from(Toml::string("foo = \"child\""));
/// let merged = merge_parent(child, parent);
/// let value = merged.find_value("foo").unwrap();
/// assert_eq!(value.as_str(), Some("child"));
/// ```
fn merge_parent(figment: Figment, parent_figment: Figment) -> Figment {
    parent_figment.merge(figment)
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
fn process_extends(
    mut figment: Figment,
    current_path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<Figment, OrthoError> {
    if let Some(base) = get_extends(&figment, current_path)? {
        let canonical =
            resolve_base_path(current_path, base).map_err(|e| file_error(current_path, e))?;
        if !canonical.is_file() {
            return Err(file_error(
                &canonical,
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "extended path is not a regular file",
                ),
            ));
        }
        let Some(parent_fig) = load_config_file_inner(&canonical, visited, stack)? else {
            return Err(file_error(
                &canonical,
                std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "extended file disappeared during load",
                ),
            ));
        };
        figment = merge_parent(figment, parent_fig);
    }
    Ok(figment)
}

/// Load configuration from a file, selecting the parser based on extension.
///
/// Returns `Ok(None)` if the file does not exist.
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::load_config_file;
/// use serde::Deserialize;
/// use std::path::Path;
///
/// #[derive(Deserialize)]
/// struct Config {
///     host: String,
/// }
///
/// # fn run() -> Result<(), ortho_config::OrthoError> {
/// if let Some(figment) = load_config_file(Path::new("config.toml"))? {
///     let config: Config = figment
///         .extract()
///         .expect("invalid configuration file");
///     assert_eq!(config.host, "localhost");
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns an [`OrthoError`] if reading or parsing the file fails.
pub fn load_config_file(path: &Path) -> Result<Option<Figment>, OrthoError> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    load_config_file_inner(path, &mut visited, &mut stack)
}

fn load_config_file_inner(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<Option<Figment>, OrthoError> {
    if !path.is_file() {
        return Ok(None);
    }
    #[cfg(windows)]
    let canonical = dunce::canonicalize(path).map_err(|e| file_error(path, e))?;
    #[cfg(not(windows))]
    let canonical = std::fs::canonicalize(path).map_err(|e| file_error(path, e))?;
    if !visited.insert(canonical.clone()) {
        let mut cycle: Vec<String> = stack.iter().map(|p| p.display().to_string()).collect();
        cycle.push(canonical.display().to_string());
        return Err(OrthoError::CyclicExtends {
            cycle: cycle.join(" -> "),
        });
    }
    stack.push(canonical.clone());
    let result = (|| {
        let data = std::fs::read_to_string(&canonical).map_err(|e| file_error(&canonical, e))?;
        let figment = parse_config_by_format(&canonical, &data)?;
        process_extends(figment, &canonical, visited, stack)
    })();
    visited.remove(&canonical);
    stack.pop();
    result.map(Some)
}

#[cfg(test)]
mod file_tests;
