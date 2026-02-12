//! Format-specific parsing utilities for configuration files.

use crate::OrthoResult;

use figment::{
    Figment,
    providers::{Format, Toml},
};
#[cfg(feature = "json5")]
use figment_json5::Json5;

use std::path::Path;

use super::error::file_error;
#[cfg(feature = "yaml")]
use super::yaml::SaphyrYaml;

/// Parse configuration data according to the file extension.
///
/// Supported formats are JSON5, YAML and TOML. The `json5` and `yaml`
/// features must be enabled for those formats to be parsed.
///
/// # Errors
///
/// Returns an [`OrthoError`] if the file contents fail to parse or if the
/// required feature is disabled.
pub(super) fn parse_config_by_format(path: &Path, data: &str) -> OrthoResult<Figment> {
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
                Figment::from(SaphyrYaml::string(path.to_path_buf(), data.to_owned()))
            }
            #[cfg(not(feature = "yaml"))]
            {
                return Err(file_error(
                    path,
                    std::io::Error::other(
                        "yaml feature disabled: enable the 'yaml' feature to support this file format",
                    ),
                ));
            }
        }
        _ => {
            // Validate TOML first so parse failures are reported with this file context
            // before Figment performs its own parse pass via `Toml::string`.
            toml::from_str::<toml::Value>(data).map_err(|e| file_error(path, e))?;
            Figment::from(Toml::string(data))
        }
    };

    Ok(figment)
}
