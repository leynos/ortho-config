//! Configuration-path selector classification and candidate construction.
//!
//! A non-empty selector value names an explicit, required path. Relative values
//! are anchored to the environment source's working directory; when the source
//! cannot report one, selection fails closed with a required error rather than
//! falling back to the process directory.

use std::path::PathBuf;
use std::sync::Arc;

use crate::OrthoError;

use super::super::ConfigDiscovery;
use super::super::candidate_set::CandidateAccumulator;
use super::super::telemetry;

/// A configured selector's value, classified.
enum SelectorValue {
    /// The variable is configured but unset in the environment.
    Unset,
    /// The variable is set to an empty value, which discovery ignores.
    Empty,
    /// The variable names a path, preserved as read.
    Accepted(std::ffi::OsString),
}

impl ConfigDiscovery {
    /// Push the configuration-path selector, reporting how it resolved.
    ///
    /// Owns the not-configured case, the accumulator side effect, and the
    /// required error for a relative value that cannot be anchored. The value
    /// classification lives in [`ConfigDiscovery::classify_selector`] so the
    /// three-way rule can be read (and changed) in one place.
    ///
    /// A relative selector is resolved against the environment source's
    /// working directory, never the process's. When the source cannot report
    /// one, the path cannot be read without bypassing that boundary, so
    /// selection fails closed with a required error instead of dropping the
    /// selector and letting automatic discovery decide.
    pub(super) fn push_selector(
        &self,
        acc: &mut CandidateAccumulator,
    ) -> (&'static str, Option<Arc<OrthoError>>) {
        let Some(env_var) = self.env_var.as_ref() else {
            return (telemetry::SELECTOR_NOT_CONFIGURED, None);
        };

        match self.classify_selector(env_var) {
            SelectorValue::Unset => (telemetry::SELECTOR_UNSET, None),
            SelectorValue::Empty => (telemetry::SELECTOR_EMPTY, None),
            SelectorValue::Accepted(value) => {
                let raw = PathBuf::from(value);
                let Some(path) = self.anchored(raw.clone()) else {
                    return (
                        telemetry::SELECTOR_DROPPED,
                        Some(unanchored_selector_error(raw)),
                    );
                };
                // A non-empty selector is explicit, so its path is required even
                // though unset and empty values reach no arm here.
                let _ = acc.push_required(path, telemetry::CANDIDATE_SELECTOR);
                (telemetry::SELECTOR_ACCEPTED, None)
            }
        }
    }

    /// Classify the configured selector's lookup result.
    ///
    /// The three non-accepting states are kept distinct because they call for
    /// different action: no selector was configured at all (handled by the
    /// caller), one was configured but the operator has not set it, or it is
    /// set to an empty value — the last being a likely mistake that discovery
    /// deliberately ignores. An accepted value keeps its `OsString` so the
    /// caller can convert it to a path without a lossy round trip.
    fn classify_selector(&self, env_var: &str) -> SelectorValue {
        match self.env_source.get(env_var) {
            None => SelectorValue::Unset,
            Some(value) if value.is_empty() => SelectorValue::Empty,
            Some(value) => SelectorValue::Accepted(value),
        }
    }
}

/// Builds the required error for a relative selector that cannot be anchored.
///
/// A relative selector must resolve against the environment source's working
/// directory. When the source cannot report one, reading the path would fall
/// back to the process's working directory, which is exactly what the injected
/// source boundary prevents. Selection therefore fails closed instead of
/// dropping the selector and letting automatic discovery choose a different
/// file.
fn unanchored_selector_error(path: PathBuf) -> Arc<OrthoError> {
    Arc::new(OrthoError::File {
        path,
        source: Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            concat!(
                "configuration path selector is relative but the environment ",
                "source cannot report a working directory"
            ),
        )),
    })
}
