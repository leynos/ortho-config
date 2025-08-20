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
#[expect(
    clippy::result_large_err,
    reason = "Error type is library specific and intentionally large"
)]
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
        #[allow(clippy::unnested_or_patterns)]
        Some("yaml") | Some("yml") => {
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
/// # use std::path::Path;
/// # use ortho_config::file::get_extends;
/// let figment = Figment::from(Toml::string("extends = \"base.toml\""));
/// let extends = get_extends(&figment, Path::new("cfg.toml")).unwrap();
/// assert_eq!(extends, Some("base.toml".to_string()));
/// ```
#[expect(
    clippy::result_large_err,
    reason = "Error type is library specific and intentionally large"
)]
fn get_extends(figment: &Figment, current_path: &Path) -> Result<Option<String>, OrthoError> {
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
            Ok(Some(base.to_owned()))
        }
        Err(e) if e.missing() => Ok(None),
        Err(e) => Err(file_error(current_path, e)),
    }
}

/// Resolve the base configuration file path relative to `current_path`.
///
/// The returned path is canonicalised.
///
/// # Examples
///
/// ```rust,ignore
/// # use std::path::Path;
/// # use ortho_config::file::resolve_base_path;
/// let path = resolve_base_path("base.toml", Path::new("dir/config.toml"))?;
/// ```
#[expect(
    clippy::result_large_err,
    reason = "propagating path resolution errors"
)]
fn resolve_base_path(base: &str, current_path: &Path) -> Result<PathBuf, OrthoError> {
    let parent = current_path.parent().ok_or_else(|| {
        file_error(
            current_path,
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Cannot determine parent directory for config file when resolving 'extends'",
            ),
        )
    })?;

    let base_path = if Path::new(base).is_absolute() {
        PathBuf::from(base)
    } else {
        parent.join(base)
    };

    std::fs::canonicalize(&base_path).map_err(|e| file_error(&base_path, e))
}

/// Load and merge the parent configuration specified by `canonical`.
///
/// The parent is loaded and then merged with `figment`, allowing the current
/// configuration to override base settings.
///
/// # Examples
///
/// ```rust,ignore
/// # use figment::{Figment, providers::{Format, Toml}};
/// # use std::collections::HashSet;
/// # use std::path::Path;
/// # use ortho_config::file::merge_parent;
/// # let figment = Figment::from(Toml::string("foo = \"bar\""));
/// # let mut visited = HashSet::new();
/// # let mut stack = Vec::new();
/// # let base = Path::new("base.toml");
/// let merged = merge_parent(figment, base, &mut visited, &mut stack)?;
/// ```
#[expect(clippy::result_large_err, reason = "propagating file loading errors")]
fn merge_parent(
    figment: Figment,
    canonical: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<Figment, OrthoError> {
    if let Some(base_fig) = load_config_file_inner(canonical, visited, stack)? {
        Ok(base_fig.merge(figment))
    } else {
        Ok(figment)
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
    figment: Figment,
    current_path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> Result<Figment, OrthoError> {
    if let Some(base) = get_extends(&figment, current_path)? {
        let canonical = resolve_base_path(&base, current_path)?;
        merge_parent(figment, &canonical, visited, stack)
    } else {
        Ok(figment)
    }
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
mod tests {
    use super::*;
    use figment::{Figment, Jail, providers::Format, providers::Toml};

    #[test]
    fn get_extends_returns_string() {
        let figment = Figment::from(Toml::string("extends = \"base.toml\""));
        let extends = get_extends(&figment, Path::new("cfg.toml")).expect("extends");
        assert_eq!(extends, Some("base.toml".to_string()));
    }

    #[test]
    fn get_extends_none_when_missing() {
        let figment = Figment::from(Toml::string("foo = \"bar\""));
        let extends = get_extends(&figment, Path::new("cfg.toml")).expect("extends");
        assert!(extends.is_none());
    }

    #[test]
    fn get_extends_errors_on_non_string() {
        let figment = Figment::from(Toml::string("extends = 1"));
        let err = get_extends(&figment, Path::new("cfg.toml")).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("must be a string"));
    }

    #[test]
    fn resolve_base_path_handles_relative_and_absolute() {
        Jail::expect_with(|j| {
            j.create_file("base.toml", "")?;
            let root = std::fs::canonicalize(".").expect("canonicalise root");
            let current = root.join("config.toml");
            let rel = resolve_base_path("base.toml", &current).expect("resolve relative");
            assert_eq!(rel, root.join("base.toml"));

            let abs_str = root.join("base.toml").to_str().expect("str").to_owned();
            let abs = resolve_base_path(&abs_str, &current).expect("resolve absolute");
            assert_eq!(abs, root.join("base.toml"));
            Ok(())
        });
    }

    #[test]
    fn merge_parent_overrides_base() {
        Jail::expect_with(|j| {
            j.create_file("base.toml", "foo = \"base\"")?;
            let canonical = std::fs::canonicalize("base.toml").expect("canonicalise base");
            let figment = Figment::from(Toml::string("foo = \"child\""));
            let mut visited = HashSet::new();
            let mut stack = Vec::new();
            let merged =
                merge_parent(figment, &canonical, &mut visited, &mut stack).expect("merge");
            let value = merged.find_value("foo").expect("foo");
            let foo = value.as_str().expect("string");
            assert_eq!(foo, "child");
            Ok(())
        });
    }
}
