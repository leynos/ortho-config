//! Generate format-specific filename alternatives for discovery.
//!
//! These helpers append JSON/JSON5 and YAML/YML variants after the canonical
//! candidate while keeping feature gates aligned with the enabled parsers.

#[cfg(any(feature = "json5", feature = "yaml"))]
use std::path::{Path, PathBuf};

use super::ConfigDiscovery;

impl ConfigDiscovery {
    /// Appends one nested filename for each extension in the supplied order.
    #[cfg(any(feature = "json5", feature = "yaml"))]
    fn push_variants_for_extensions(
        candidates: &mut Vec<PathBuf>,
        nested: &Path,
        stem: &str,
        extensions: &[&str],
    ) {
        for ext in extensions {
            let filename = format!("{stem}.{ext}");
            candidates.push(nested.join(&filename));
        }
    }

    /// Adds JSON and JSON5 alternatives after the canonical filename.
    #[cfg(feature = "json5")]
    pub(super) fn push_json_variant_candidates(
        candidates: &mut Vec<PathBuf>,
        nested: &Path,
        stem: &str,
    ) {
        Self::push_variants_for_extensions(candidates, nested, stem, &["json", "json5"]);
    }

    /// Adds YAML and YML alternatives after the canonical filename.
    #[cfg(feature = "yaml")]
    pub(super) fn push_yaml_variant_candidates(
        candidates: &mut Vec<PathBuf>,
        nested: &Path,
        stem: &str,
    ) {
        Self::push_variants_for_extensions(candidates, nested, stem, &["yaml", "yml"]);
    }
}
