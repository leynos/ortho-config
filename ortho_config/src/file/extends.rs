//! Inheritance (`extends`) parsing and merge orchestration.

use crate::OrthoResult;

use figment::Figment;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::error::{file_error, invalid_data, invalid_input, not_found};
use super::loader::load_config_file_inner;
use super::path::resolve_base_path;

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
pub(super) fn get_extends(figment: &Figment, current_path: &Path) -> OrthoResult<Option<PathBuf>> {
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
pub(super) fn merge_parent(figment: Figment, parent_figment: Figment) -> Figment {
    parent_figment.merge(figment)
}

/// Apply inheritance using the `extends` key.
///
/// The referenced file is loaded first and the current [`Figment`] is merged
/// over it. Cycles are detected using `visited`.
///
/// # Errors
///
/// Returns an [`crate::OrthoError`] if the extended file fails to load or the
/// `extends` key is malformed.
pub(super) fn process_extends(
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
