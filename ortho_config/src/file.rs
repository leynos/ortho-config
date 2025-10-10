//! Helpers for reading configuration files into Figment.

use crate::{OrthoError, OrthoResult};
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
fn file_error(
    path: &Path,
    err: impl Into<Box<dyn Error + Send + Sync>>,
) -> std::sync::Arc<OrthoError> {
    std::sync::Arc::new(OrthoError::File {
        path: path.to_path_buf(),
        source: err.into(),
    })
}

fn invalid_input(path: &Path, msg: impl Into<String>) -> std::sync::Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::InvalidInput, msg.into()),
    )
}

fn invalid_data(path: &Path, msg: impl Into<String>) -> std::sync::Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::InvalidData, msg.into()),
    )
}

fn not_found(path: &Path, msg: impl Into<String>) -> std::sync::Arc<OrthoError> {
    file_error(
        path,
        std::io::Error::new(std::io::ErrorKind::NotFound, msg.into()),
    )
}

/// Canonicalise `p` using platform-specific rules.
///
/// Returns an absolute, normalised path with symlinks resolved.
///
/// On Windows the [`dunce`] crate is used to avoid introducing UNC prefixes
/// in diagnostic messages.
///
/// # Errors
///
/// Returns an [`OrthoError`] if canonicalisation fails.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::Path;
///
/// # fn run() -> ortho_config::OrthoResult<()> {
/// let p = Path::new("config.toml");
/// let c = ortho_config::file::canonicalise(p)?;
/// assert!(c.is_absolute());
/// # Ok(())
/// # }
/// ```
pub fn canonicalise(p: &Path) -> OrthoResult<PathBuf> {
    #[cfg(windows)]
    {
        dunce::canonicalize(p).map_err(|e| file_error(p, e))
    }
    #[cfg(not(windows))]
    {
        std::fs::canonicalize(p).map_err(|e| file_error(p, e))
    }
}

/// Normalise a canonical path for case-insensitive cycle detection.
///
/// The loader stores normalised keys in its visited set to ensure that files
/// referenced with different casing are treated as the same node when the
/// filesystem ignores case differences. On strictly case-sensitive platforms
/// the path is returned unchanged.
///
/// # Examples
///
/// ```rust,ignore
/// use std::path::Path;
///
/// let canonical = Path::new("/configs/Config.toml");
/// let key = ortho_config::file::normalise_cycle_key(canonical);
/// // On Windows and macOS the key is lower-cased so variants like
/// // "/configs/config.toml" do not bypass cycle detection.
/// ```
fn normalise_cycle_key(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        // Windows treats ASCII letters case-insensitively, but leaves other
        // code points unchanged. Fold only the ASCII range so we mirror the
        // filesystem's comparison rules without mangling non-ASCII names.
        let mut lowered = Vec::with_capacity(path.as_os_str().len());
        for unit in path.as_os_str().encode_wide() {
            if (u16::from(b'A')..=u16::from(b'Z')).contains(&unit) {
                lowered.push(unit + 32);
            } else {
                lowered.push(unit);
            }
        }
        return PathBuf::from(OsString::from_wide(&lowered));
    }

    #[cfg(target_os = "macos")]
    {
        use std::ffi::OsString;

        let lowered = path.as_os_str().to_string_lossy().to_lowercase();
        return PathBuf::from(OsString::from(lowered));
    }

    path.to_path_buf()
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
fn parse_config_by_format(path: &Path, data: &str) -> OrthoResult<Figment> {
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
                return Err::<_, std::sync::Arc<OrthoError>>(file_error(
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
                return Err::<_, std::sync::Arc<OrthoError>>(file_error(
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
/// Returns `Ok(None)` if the key is absent. Empty strings are rejected with an
/// error.
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
fn get_extends(figment: &Figment, current_path: &Path) -> OrthoResult<Option<PathBuf>> {
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
                invalid_data(
                    current_path,
                    format!("'extends' key must be a string, but found type: {actual_type}"),
                )
            })?;
            if base.is_empty() {
                return Err(invalid_data(
                    current_path,
                    "'extends' key must be a non-empty string",
                ));
            }
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
/// ```rust,ignore
/// # use std::path::{Path, PathBuf};
/// # use ortho_config::file::resolve_base_path;
/// # fn run() -> ortho_config::OrthoResult<()> {
/// let current = Path::new("/tmp/config.toml");
/// let base = PathBuf::from("base.toml");
/// let canonical = resolve_base_path(current, base)?;
/// assert!(canonical.ends_with("base.toml"));
/// # Ok(())
/// # }
/// ```
fn resolve_base_path(current_path: &Path, base: PathBuf) -> OrthoResult<PathBuf> {
    let parent = current_path.parent().ok_or_else(|| {
        invalid_input(
            current_path,
            "Cannot determine parent directory for config file when resolving 'extends'",
        )
    })?;
    let base = if base.is_absolute() {
        base
    } else {
        parent.join(base)
    };
    canonicalise(&base)
}

/// Merge `figment` over its parent configuration.
///
/// The parent is used as the base configuration with `figment` overriding its
/// values.
///
/// # Examples
///
/// ```rust,ignore
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
) -> OrthoResult<Figment> {
    if let Some(base) = get_extends(&figment, current_path)? {
        let canonical = resolve_base_path(current_path, base)?;
        if !canonical.is_file() {
            return Err(invalid_input(
                &canonical,
                "extended path is not a regular file",
            ));
        }
        let Some(parent_fig) = load_config_file_inner(&canonical, visited, stack)? else {
            return Err(not_found(
                &canonical,
                "extended file disappeared during load",
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
/// # fn run() -> ortho_config::OrthoResult<()> {
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
pub fn load_config_file(path: &Path) -> OrthoResult<Option<Figment>> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    load_config_file_inner(path, &mut visited, &mut stack)
}

fn load_config_file_inner(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> OrthoResult<Option<Figment>> {
    if !path.is_file() {
        return Ok(None);
    }
    let canonical = canonicalise(path)?;
    let normalised = normalise_cycle_key(&canonical);
    if !visited.insert(normalised.clone()) {
        let mut cycle: Vec<String> = stack.iter().map(|p| p.display().to_string()).collect();
        cycle.push(canonical.display().to_string());
        return Err(std::sync::Arc::new(OrthoError::CyclicExtends {
            cycle: cycle.join(" -> "),
        }));
    }
    stack.push(canonical.clone());
    let result = (|| {
        let data = std::fs::read_to_string(&canonical).map_err(|e| file_error(&canonical, e))?;
        let figment = parse_config_by_format(&canonical, &data)?;
        process_extends(figment, &canonical, visited, stack)
    })();
    visited.remove(&normalised);
    stack.pop();
    result.map(Some)
}

#[cfg(test)]
mod file_tests;
