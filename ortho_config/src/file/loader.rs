//! Runtime loading entrypoints for configuration files and extends chains.

use camino::Utf8PathBuf;
use serde_json::Value as JsonValue;

use crate::{OrthoError, OrthoMergeExt, OrthoResult};

use figment::Figment;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::error::{file_error, invalid_input};
use super::extends::{get_extends, process_extends};
use super::parser::parse_config_by_format;
use super::path::{canonicalise, normalise_cycle_key, resolve_base_path};

/// Values from a file inheritance chain, ordered ancestor-first.
///
/// When a configuration file uses `extends`, this struct captures each file's
/// values separately so that declarative merge strategies (such as append for
/// vectors) can be applied across the inheritance chain.
///
/// The first entry is the root ancestor; the last is the directly-loaded file.
#[derive(Debug, Default)]
pub struct FileLayerChain {
    /// Ordered (ancestor-first) JSON values with their source paths.
    pub values: Vec<(JsonValue, Utf8PathBuf)>,
}

/// Remove the `extends` key from a JSON value to avoid polluting the final
/// configuration.
fn strip_extends_key(value: &mut JsonValue) {
    if let JsonValue::Object(map) = value {
        map.remove("extends");
    }
}

/// Convert a canonical path to a UTF-8 path, falling back to lossy conversion.
fn to_utf8_path(canonical: &Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(canonical.to_path_buf())
        .unwrap_or_else(|p| Utf8PathBuf::from(p.to_string_lossy().into_owned()))
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

fn with_cycle_detection<T, F>(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
    operation: F,
) -> OrthoResult<Option<T>>
where
    F: FnOnce(&Path, &mut HashSet<PathBuf>, &mut Vec<PathBuf>) -> OrthoResult<T>,
{
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
    let result = operation(&canonical, visited, stack);
    visited.remove(&normalised);
    stack.pop();
    result.map(Some)
}

pub(super) fn load_config_file_inner(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> OrthoResult<Option<Figment>> {
    with_cycle_detection(
        path,
        visited,
        stack,
        |canonical, visited_paths, stack_paths| {
            let data = std::fs::read_to_string(canonical).map_err(|e| file_error(canonical, e))?;
            let figment = parse_config_by_format(canonical, &data)?;
            process_extends(figment, canonical, visited_paths, stack_paths)
        },
    )
}

/// Load configuration from a file as a chain of layers for declarative merging.
///
/// Unlike [`load_config_file`], this function preserves each file in an
/// `extends` chain as a separate layer. This allows the declarative merge
/// system to apply its merge strategies (for example, append for vectors)
/// across the inheritance chain.
///
/// Returns `Ok(None)` if the file does not exist.
///
/// # Examples
///
/// ```rust,no_run
/// use ortho_config::file::load_config_file_as_chain;
/// use std::path::Path;
///
/// # fn run() -> ortho_config::OrthoResult<()> {
/// if let Some(chain) = load_config_file_as_chain(Path::new("config.toml"))? {
///     // chain.values contains one entry per file in the extends chain
///     for (value, path) in &chain.values {
///         println!("Loaded {} with {} keys", path, value.as_object().map_or(0, |o| o.len()));
///     }
/// }
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns an [`OrthoError`] if reading or parsing the file fails.
pub fn load_config_file_as_chain(path: &Path) -> OrthoResult<Option<FileLayerChain>> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    with_cycle_detection(path, &mut visited, &mut stack, load_chain_for_file)
}

fn load_chain_for_file(
    canonical: &Path,
    visited: &mut HashSet<PathBuf>,
    stack: &mut Vec<PathBuf>,
) -> OrthoResult<FileLayerChain> {
    let data = std::fs::read_to_string(canonical).map_err(|e| file_error(canonical, e))?;
    let figment = parse_config_by_format(canonical, &data)?;

    // Extract this file's JSON value.
    let mut value: JsonValue = figment.extract().into_ortho_merge()?;
    strip_extends_key(&mut value);
    let utf8_path = to_utf8_path(canonical);

    // Get parent chain if extends is present.
    let parent_chain = if let Some(base) = get_extends(&figment, canonical)? {
        let base_canonical = resolve_base_path(canonical, base)?;
        if !base_canonical.is_file() {
            return Err(invalid_input(
                &base_canonical,
                "extended path is not a regular file",
            ));
        }
        with_cycle_detection(&base_canonical, visited, stack, load_chain_for_file)?
    } else {
        None
    };

    // Build the full chain: parent chain + this file.
    let chain = match parent_chain {
        Some(mut parent) => {
            parent.values.push((value, utf8_path));
            parent
        }
        None => FileLayerChain {
            values: vec![(value, utf8_path)],
        },
    };

    Ok(chain)
}
