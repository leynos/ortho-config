//! Helpers for working with declarative configuration fixtures.

use cap_std::fs::Dir;
use std::io;
use thiserror::Error;

#[derive(Debug, Copy, Clone)]
pub(crate) struct ConfigCopyParams<'a> {
    pub(crate) source: &'a Dir,
    pub(crate) source_name: &'a str,
    pub(crate) target_name: &'a str,
}

/// Errors raised while preparing fixture configuration for scenarios.
#[derive(Debug, Error)]
pub enum SampleConfigError {
    /// Failed to open the directory containing sample configurations.
    #[error("failed to open hello world sample config directory: {path}")]
    OpenConfigDir {
        /// Path to the directory that could not be opened.
        path: String,
        /// Underlying IO error reported by the filesystem.
        #[source]
        source: io::Error,
    },
    /// Encountered an invalid file name when selecting a sample.
    #[error("invalid hello world sample config name {name}")]
    InvalidName {
        /// Name that violated the sample selection rules.
        name: String,
    },
    /// Failed to open an individual sample configuration file.
    #[error("failed to open hello world sample config {name}")]
    OpenSample {
        /// Name of the sample configuration involved in the failure.
        name: String,
        /// Source error raised while opening the sample.
        #[source]
        source: io::Error,
    },
    /// Failed to read from an opened sample configuration file.
    #[error("failed to read hello world sample config {name}")]
    ReadSample {
        /// Name of the sample configuration being read.
        name: String,
        /// Source error raised during file read.
        #[source]
        source: io::Error,
    },
    /// Failed to write a sample configuration into the scenario directory.
    #[error("failed to write hello world sample config {name}")]
    WriteSample {
        /// Name of the sample configuration that could not be written.
        name: String,
        /// Source error raised during file write.
        #[source]
        source: io::Error,
    },
}

pub(crate) fn is_simple_filename(name: &str) -> bool {
    !name.is_empty() && !name.chars().any(std::path::is_separator)
}

pub(crate) fn ensure_simple_filename(name: &str) -> Result<(), SampleConfigError> {
    if is_simple_filename(name) {
        Ok(())
    } else {
        Err(SampleConfigError::InvalidName {
            name: name.to_owned(),
        })
    }
}

pub(crate) fn parse_extends(contents: &str) -> Vec<String> {
    let document: toml::Value = match toml::from_str(contents) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    match document.get("extends") {
        Some(toml::Value::String(path)) => extract_single_path(path),
        Some(toml::Value::Array(values)) => extract_multiple_paths(values),
        _ => Vec::new(),
    }
}

fn extract_single_path(path: &str) -> Vec<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_owned()]
    }
}

fn extract_multiple_paths(values: &[toml::Value]) -> Vec<String> {
    values.iter().filter_map(extract_string_value).collect()
}

fn extract_string_value(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(path) => {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_extends_single_string() {
        let out = super::parse_extends(r#"extends = "base.toml""#);
        assert_eq!(out, vec!["base.toml"]);
    }

    #[test]
    fn parse_extends_array_mixed_types_filters_non_strings() {
        let out = super::parse_extends(r#"extends = ["a.toml", 42, " b . toml ", "", { k = "v" }]"#);
        assert_eq!(out, vec!["a.toml", "b . toml"]);
    }

    #[test]
    fn parse_extends_ignores_malformed_toml() {
        let out = super::parse_extends(r#"extends = [ "a.toml", ""#);
        assert!(out.is_empty());
    }

    #[test]
    fn parse_extends_nested_array_filters_deeper_levels() {
        let out = super::parse_extends(r#"extends = [["base.toml"], " extra.toml ", ""]"#);
        assert_eq!(out, vec!["extra.toml"]);
    }

    #[test]
    fn ensure_simple_filename_rejects_paths() {
        assert!(super::ensure_simple_filename("config.toml").is_ok());
        assert!(super::ensure_simple_filename("../config.toml").is_err());
    }
}
