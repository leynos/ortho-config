//! Helpers that compose the configuration layers for the CLI example.
//!
//! These routines glue together CLI flags, discovered configuration files,
//! and defaults so the higher-level module can focus on validation.

use std::{ffi::OsString, path::Path, sync::Arc};

use ortho_config::OrthoError;

use crate::error::HelloWorldError;

use super::{
    GlobalArgs,
    discovery::discover_config_figment,
    overrides::{FileOverrides, Overrides},
};

#[derive(serde::Serialize)]
pub(super) struct FileLayer {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) is_excited: Option<bool>,
}

pub(crate) fn build_overrides<'a>(
    globals: &'a GlobalArgs,
    salutations: Option<Vec<String>>,
    file_overrides: Option<&FileOverrides>,
    config_override: Option<&Path>,
) -> Overrides<'a> {
    let file_is_excited = file_excited_value(file_overrides, config_override);
    Overrides {
        recipient: globals.recipient.as_ref(),
        salutations,
        is_excited: globals.is_excited.then_some(true).or(file_is_excited),
        is_quiet: globals.is_quiet,
    }
}

pub(crate) fn build_cli_args(config_override: Option<&Path>) -> Vec<OsString> {
    let binary = std::env::args_os()
        .next()
        .unwrap_or_else(|| OsString::from("hello-world"));
    let mut args = vec![binary];
    if let Some(path) = config_override {
        args.push(OsString::from("--config"));
        args.push(path.as_os_str().to_os_string());
    }
    args
}

pub(crate) fn trimmed_salutations(globals: &GlobalArgs) -> Option<Vec<String>> {
    (!globals.salutations.is_empty()).then(|| {
        globals
            .salutations
            .iter()
            .map(|value| value.trim().to_owned())
            .collect()
    })
}

/// Resolves the `is_excited` value from configuration sources with priority fallback.
///
/// Attempts to extract the value from `config_override` first. If that source is absent,
/// invalid, or parsing fails, falls back to the value in `file_overrides`. Returns `None`
/// only if both sources are absent or yield no value.
pub(crate) fn file_excited_value(
    file_overrides: Option<&FileOverrides>,
    config_override: Option<&Path>,
) -> Option<bool> {
    config_override
        .and_then(|path| {
            ortho_config::load_config_file(path)
                .ok()?
                .and_then(|fig| fig.extract_inner::<bool>("is_excited").ok())
        })
        .or_else(|| file_overrides.and_then(|file| file.is_excited))
}

pub(crate) fn load_config_overrides() -> Result<Option<FileOverrides>, HelloWorldError> {
    if let Some(figment) = discover_config_figment()? {
        let overrides: FileOverrides = figment
            .extract()
            .map_err(|err| HelloWorldError::Configuration(Arc::new(OrthoError::merge(err))))?;
        Ok(Some(overrides))
    } else {
        Ok(None)
    }
}
