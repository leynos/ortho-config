//! Replay declarative `CsvEnv` transforms for injected variable scans.
//!
//! This module owns the path that cannot delegate to Figment's process-backed
//! iterator. It retains only inspectable transform state, rejecting opaque
//! closures rather than silently changing a configuration key's nesting.

use super::{CsvEnv, KeyTransform};
use figment::error::Error;
use uncased::{Uncased, UncasedStr};

impl CsvEnv {
    /// Scan and replay the declarative key transforms for an injected source.
    ///
    /// An opaque closure is rejected before scanning because applying an
    /// approximation would silently produce a differently nested key.
    pub(super) fn injected_entries(&self) -> Result<Vec<(Uncased<'static>, String)>, Box<Error>> {
        if matches!(self.options.key_transform, KeyTransform::Opaque) {
            return Err(Box::new(Error::from(
                "CsvEnv cannot use an injected ScanEnvSource after map or filter_map",
            )));
        }

        let Some(source) = &self.options.source else {
            return Ok(Vec::new());
        };

        Ok(source
            .scan()
            .into_iter()
            .filter_map(|(raw_key, value)| {
                self.transform_injected_key(&raw_key.to_string_lossy())
                    .map(|transformed_key| {
                        (
                            Uncased::from(transformed_key),
                            value.to_string_lossy().to_string(),
                        )
                    })
            })
            .collect())
    }

    /// Reproduce Figment's prefix, case, split, trim, and empty-part rules.
    ///
    /// `None` means the source key does not participate in the provider, not
    /// that an error occurred; this matches Figment dropping such keys.
    fn transform_injected_key(&self, raw_key: &str) -> Option<String> {
        let trimmed_key = raw_key.trim();
        let stripped_key = self.strip_prefix(trimmed_key)?;
        let uppercased_key = self
            .options
            .uppercase
            .is_enabled()
            .then(|| stripped_key.to_ascii_uppercase());
        let split_input = uppercased_key.as_deref().unwrap_or(stripped_key);
        let split_key = self.options.split_pattern.as_ref().map_or_else(
            || split_input.to_owned(),
            |pattern| split_input.replace(pattern, "."),
        );
        let trimmed_split_key = split_key.trim();

        if trimmed_split_key.split('.').any(str::is_empty) {
            return None;
        }

        Some(if self.options.lowercase.is_enabled() {
            trimmed_split_key.to_ascii_lowercase()
        } else {
            trimmed_split_key.to_owned()
        })
    }

    /// Remove the configured prefix with Figment's case-insensitive matching.
    ///
    /// The slice comes from `key`, preserving the original bytes for the later
    /// case transforms while avoiding an allocation when no prefix is set.
    fn strip_prefix<'a>(&self, key: &'a str) -> Option<&'a str> {
        self.options.prefix.as_deref().map_or_else(
            || Some(key),
            |prefix| {
                UncasedStr::new(key)
                    .starts_with(prefix)
                    .then(|| key.get(prefix.len()..))
                    .flatten()
            },
        )
    }
}
